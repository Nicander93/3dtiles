//! Spawn `processor` child, Job Object / process-group cancel, serial queue (T03/T04).

use crate::artifact_store::ArtifactStore;
use crate::db::now_secs;
use crate::task_store::TaskStore;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Default grace period after cooperative cancel before force-killing the process tree.
pub const CANCEL_GRACE_SECS: u64 = 10;

#[derive(Clone, Default)]
pub struct ProcessManager {
  inner: Arc<Mutex<HashMap<String, ActiveProc>>>,
  /// Single-flight serial runner.
  scheduler: Arc<Mutex<Scheduler>>,
}

#[derive(Default)]
struct Scheduler {
  running: Option<String>,
  /// Wake the scheduler loop.
  kick: Option<std::sync::mpsc::Sender<()>>,
}

struct ActiveProc {
  child: Child,
  stdin: Option<ChildStdin>,
  #[cfg(windows)]
  job: Option<JobHandle>,
  force_kill: Arc<AtomicBool>,
  /// Set once result event received / commit succeeded path known.
  committed: Arc<AtomicBool>,
}

#[cfg(windows)]
struct JobHandle(*mut std::ffi::c_void);
#[cfg(windows)]
unsafe impl Send for JobHandle {}
#[cfg(windows)]
impl Drop for JobHandle {
  fn drop(&mut self) {
    if !self.0.is_null() {
      unsafe {
        CloseHandle(self.0);
      }
    }
  }
}

impl ProcessManager {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn processor_available() -> bool {
    resolve_processor_bin().is_some()
  }

  pub fn processor_bin() -> Option<PathBuf> {
    resolve_processor_bin()
  }

  /// Bundled `resources/runtime` that should use packaged semantics (no Docker fallback).
  pub fn packaged_runtime_root() -> Option<PathBuf> {
    packaged_runtime_root_from_exe()
  }

  pub fn apply_runtime_env(command: &mut Command) {
    if let Some(runtime) = Self::packaged_runtime_root() {
      command.env("GEOFORGE_RUNTIME_ROOT", &runtime);
      command.env("GEOFORGE_PACKAGED", "1");
    }
  }

  /// Enqueue / kick serial runner for a queued task id.
  pub fn enqueue(
    &self,
    tasks: TaskStore,
    artifacts: ArtifactStore,
    task_id: String,
    data_dir: PathBuf,
  ) {
    self.ensure_scheduler(tasks.clone(), artifacts.clone(), data_dir.clone());
    let _ = tasks.append_log(&task_id, "[desktop] queued (serial executor)");
    self.kick();
  }

  /// Legacy name used by submit_task — now serial.
  pub fn spawn_task(
    &self,
    tasks: TaskStore,
    artifacts: ArtifactStore,
    task_id: String,
    data_dir: PathBuf,
  ) {
    self.enqueue(tasks, artifacts, task_id, data_dir);
  }

  fn ensure_scheduler(&self, tasks: TaskStore, artifacts: ArtifactStore, data_dir: PathBuf) {
    let mut sched = self.scheduler.lock();
    if sched.kick.is_some() {
      return;
    }
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    sched.kick = Some(tx);
    let mgr = self.clone();
    thread::spawn(move || {
      loop {
        let _ = rx.recv();
        // Drain extra kicks
        while rx.try_recv().is_ok() {}
        loop {
          {
            let sched = mgr.scheduler.lock();
            if sched.running.is_some() {
              break;
            }
          }
          let next = match tasks.next_queued() {
            Ok(Some(t)) => t,
            _ => break,
          };
          // Honour cancel before start
          if next.cancel_requested || next.status == "cancelled" {
            let _ = tasks.update_fields(&next.id, |t| {
              t.status = "cancelled".into();
              t.stage = "cancelled".into();
              t.finished_at = Some(now_secs());
            });
            continue;
          }
          {
            let mut sched = mgr.scheduler.lock();
            sched.running = Some(next.id.clone());
          }
          let tid = next.id.clone();
          let result = run_processor_task(&mgr, &tasks, &artifacts, &tid, &data_dir);
          if let Err(e) = result {
            let _ = tasks.append_log(&tid, &format!("[processor] error: {e}"));
            let _ = tasks.update_fields(&tid, |t| {
              if t.status != "cancelled" && t.status != "cancelling" {
                // Keep first useful error if already set
                if t.error.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                  t.error = Some(e);
                } else if !e.contains("exited") {
                  // prefer more specific existing error; append exit note
                  let prev = t.error.clone().unwrap_or_default();
                  t.error = Some(format!("{prev}; {e}"));
                }
                t.status = "failed".into();
                t.stage = "failed".into();
              } else {
                t.status = "cancelled".into();
                t.stage = "cancelled".into();
              }
              t.finished_at = Some(now_secs());
            });
          }
          mgr.inner.lock().remove(&tid);
          {
            let mut sched = mgr.scheduler.lock();
            sched.running = None;
          }
        }
      }
    });
  }

  fn kick(&self) {
    if let Some(tx) = self.scheduler.lock().kick.as_ref() {
      let _ = tx.send(());
    }
  }

  /// Cooperative cancel: stdin "cancel", wait grace, then kill process tree. Returns immediately.
  pub fn cancel(&self, task_id: &str) -> bool {
    let grace = Duration::from_secs(
      std::env::var("GEOFORGE_CANCEL_GRACE_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(CANCEL_GRACE_SECS),
    );
    let mut map = self.inner.lock();
    let Some(proc) = map.get_mut(task_id) else {
      return false;
    };
    if proc.committed.load(Ordering::SeqCst) {
      // Publish already done — do not force-kill as cancel of result
      return true;
    }
    if let Some(ref mut stdin) = proc.stdin {
      let _ = writeln!(stdin, "cancel");
      let _ = stdin.flush();
    }
    let force = proc.force_kill.clone();
    let committed = proc.committed.clone();
    #[cfg(windows)]
    let job_raw = proc.job.as_ref().map(|j| j.0 as isize);
    #[cfg(windows)]
    let pid = proc.child.id();
    #[cfg(unix)]
    let pid = proc.child.id() as i32;
    drop(map);

    let mgr = self.clone();
    let tid = task_id.to_string();
    thread::spawn(move || {
      let deadline = Instant::now() + grace;
      while Instant::now() < deadline {
        if committed.load(Ordering::SeqCst) {
          return;
        }
        // Child gone?
        {
          let mut map = mgr.inner.lock();
          if let Some(p) = map.get_mut(&tid) {
            if let Ok(Some(_)) = p.child.try_wait() {
              return;
            }
          } else {
            return;
          }
        }
        thread::sleep(Duration::from_millis(100));
      }
      if committed.load(Ordering::SeqCst) {
        return;
      }
      force.store(true, Ordering::SeqCst);
      #[cfg(windows)]
      {
        if let Some(raw) = job_raw {
          unsafe {
            TerminateJobObject(raw as *mut std::ffi::c_void, 1);
          }
        } else {
          let mut map = mgr.inner.lock();
          if let Some(p) = map.get_mut(&tid) {
            let _ = p.child.kill();
          }
        }
        let _ = pid;
      }
      #[cfg(unix)]
      {
        unsafe {
          libc_kill(-(pid as i32), 15);
        }
        thread::sleep(Duration::from_secs(2));
        unsafe {
          libc_kill(-(pid as i32), 9);
        }
        let mut map = mgr.inner.lock();
        if let Some(p) = map.get_mut(&tid) {
          let _ = p.child.kill();
        }
      }
      #[cfg(not(any(windows, unix)))]
      {
        let mut map = mgr.inner.lock();
        if let Some(p) = map.get_mut(&tid) {
          let _ = p.child.kill();
        }
      }
    });
    true
  }

  pub fn notify_idle(&self) {
    self.kick();
  }
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
  fn CreateJobObjectW(lpJobAttributes: *mut std::ffi::c_void, lpName: *const u16) -> *mut std::ffi::c_void;
  fn AssignProcessToJobObject(hJob: *mut std::ffi::c_void, hProcess: *mut std::ffi::c_void) -> i32;
  fn TerminateJobObject(hJob: *mut std::ffi::c_void, uExitCode: u32) -> i32;
  fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
}

#[cfg(unix)]
fn libc_kill(pid: i32, sig: i32) {
  extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
  }
  unsafe {
    let _ = kill(pid, sig);
  }
}

fn packaged_runtime_root_from_exe() -> Option<PathBuf> {
  let exe = std::env::current_exe().ok()?;
  let dir = exe.parent()?;
  let runtime = dir.join("resources").join("runtime");
  if !runtime.is_dir() {
    return None;
  }
  let has_converter = runtime.join("converter").join("_3dtile.exe").is_file()
    || runtime.join("converter").join("_3dtile").is_file();
  let path = runtime.to_string_lossy();
  let in_tauri_target = path.contains("src-tauri\\target") || path.contains("src-tauri/target");
  if has_converter || !in_tauri_target {
    Some(runtime)
  } else {
    None
  }
}

fn resolve_processor_bin() -> Option<PathBuf> {
  if let Ok(p) = std::env::var("GEOFORGE_PROCESSOR") {
    let pb = PathBuf::from(&p);
    if pb.is_file() {
      return Some(pb);
    }
  }
  if let Ok(exe) = std::env::current_exe() {
    if let Some(dir) = exe.parent() {
      for name in ["processor", "processor.exe"] {
        let cand = dir.join(name);
        if cand.is_file() {
          return Some(cand);
        }
      }
      for rel in [
        "../../../target/debug/processor",
        "../../../target/debug/processor.exe",
        "../../../target/release/processor",
        "../../../target/release/processor.exe",
      ] {
        let cand = dir.join(rel);
        if cand.is_file() {
          return Some(cand);
        }
      }
    }
  }
  if let Ok(cwd) = std::env::current_dir() {
    for anc in cwd.ancestors().take(8) {
      for sub in [
        "target/debug/processor",
        "target/debug/processor.exe",
        "target/release/processor",
        "target/release/processor.exe",
      ] {
        let cand = anc.join(sub);
        if cand.is_file() {
          return Some(cand);
        }
      }
    }
  }
  None
}

fn run_processor_task(
  mgr: &ProcessManager,
  tasks: &TaskStore,
  artifacts: &ArtifactStore,
  task_id: &str,
  data_dir: &Path,
) -> Result<(), String> {
  // Re-check cancel at claim time
  let task = tasks
    .get(task_id)?
    .ok_or_else(|| "task missing".to_string())?;
  if task.cancel_requested {
    let _ = tasks.update_fields(task_id, |t| {
      t.status = "cancelled".into();
      t.stage = "cancelled".into();
      t.finished_at = Some(now_secs());
    });
    return Ok(());
  }

  let bin = resolve_processor_bin().ok_or_else(|| {
    "找不到 processor 组件。请修复安装或设置 GEOFORGE_PROCESSOR。".to_string()
  })?;

  let _ = tasks.append_log(
    task_id,
    &format!("[processor] spawning {}", bin.display()),
  );

  let tasks_dir = data_dir.join("processor-tasks");
  std::fs::create_dir_all(&tasks_dir).map_err(|e| e.to_string())?;
  let task_json_path = tasks_dir.join(format!("{task_id}.json"));
  let config = json!({
    "schemaVersion": 1,
    "taskId": task_id,
    "operation": task.operation,
    "input": { "path": task.input.path },
    "output": { "path": task.output.path },
    "options": task.options,
  });
  std::fs::write(
    &task_json_path,
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?,
  )
  .map_err(|e| e.to_string())?;

  let mut command = Command::new(&bin);
  command
    .arg("run")
    .arg("--task")
    .arg(&task_json_path)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

  ProcessManager::apply_runtime_env(&mut command);

  #[cfg(unix)]
  {
    use std::os::unix::process::CommandExt;
    unsafe {
      command.pre_exec(|| {
        extern "C" {
          fn setpgid(pid: i32, pgid: i32) -> i32;
        }
        let _ = setpgid(0, 0);
        Ok(())
      });
    }
  }

  let mut child = command
    .spawn()
    .map_err(|e| format!("spawn processor failed: {e}"))?;

  #[cfg(windows)]
  let job = unsafe {
    let h = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
    if !h.is_null() {
      use std::os::windows::io::AsRawHandle;
      let ok = AssignProcessToJobObject(h, child.as_raw_handle() as *mut _);
      if ok == 0 {
        let _ = CloseHandle(h);
        None
      } else {
        Some(JobHandle(h))
      }
    } else {
      None
    }
  };

  let pid = child.id() as i64;
  let stdin = child.stdin.take();
  let stdout = child.stdout.take().ok_or_else(|| "no stdout".to_string())?;
  let stderr = child.stderr.take();
  let committed = Arc::new(AtomicBool::new(false));
  let force_kill = Arc::new(AtomicBool::new(false));

  if let Some(err) = stderr {
    let tasks_c = tasks.clone();
    let tid = task_id.to_string();
    thread::spawn(move || {
      for line in BufReader::new(err).lines().flatten() {
        let _ = tasks_c.append_log(&tid, &format!("[processor:stderr] {line}"));
      }
    });
  }

  {
    let mut map = mgr.inner.lock();
    map.insert(
      task_id.to_string(),
      ActiveProc {
        child,
        stdin,
        #[cfg(windows)]
        job,
        force_kill: force_kill.clone(),
        committed: committed.clone(),
      },
    );
  }

  let _ = tasks.update_fields(task_id, |t| {
    t.status = "running".into();
    t.stage = "scan".into();
    t.started_at = Some(now_secs());
    t.pid = Some(pid);
    let mut prog = t.progress.as_object().cloned().unwrap_or_default();
    prog.insert("executor".into(), json!("processor"));
    t.progress = Value::Object(prog);
  });

  let mut result_path: Option<String> = None;
  let reader = BufReader::new(stdout);
  for line in reader.lines().flatten() {
    let line = line.trim().to_string();
    if line.is_empty() {
      continue;
    }
    if let Ok(Some(t)) = tasks.get(task_id) {
      if t.cancel_requested && !committed.load(Ordering::SeqCst) {
        mgr.cancel(task_id);
      }
    }
    match serde_json::from_str::<Value>(&line) {
      Ok(ev) => {
        apply_event(tasks, task_id, &ev, &mut result_path);
        if ev.get("type").and_then(|v| v.as_str()) == Some("result") {
          committed.store(true, Ordering::SeqCst);
        }
      }
      Err(_) => {
        let _ = tasks.append_log(task_id, &format!("[processor:raw] {line}"));
      }
    }
  }

  let exit_code = {
    let mut map = mgr.inner.lock();
    if let Some(mut proc) = map.remove(task_id) {
      match proc.child.wait() {
        Ok(st) => st.code().unwrap_or(1),
        Err(_) => 1,
      }
    } else {
      1
    }
  };

  thread::sleep(Duration::from_millis(50));

  let task_now = tasks.get(task_id)?.ok_or_else(|| "task missing".to_string())?;

  // Success + result: never rewrite to cancelled
  if exit_code == 0 && (result_path.is_some() || committed.load(Ordering::SeqCst)) {
    let out_path = result_path.clone().unwrap_or_else(|| task_now.output.path.clone());
    let _ = tasks.update_fields(task_id, |t| {
      t.status = "succeeded".into();
      t.stage = "done".into();
      t.finished_at = Some(now_secs());
      let mut prog = t.progress.as_object().cloned().unwrap_or_default();
      prog.insert("path".into(), json!(out_path));
      t.progress = Value::Object(prog);
    });
    register_or_report(tasks, artifacts, task_id, &out_path)?;
    return Ok(());
  }

  if task_now.cancel_requested || exit_code == 2 || force_kill.load(Ordering::SeqCst) {
    // Only if not already succeeded
    if task_now.status != "succeeded" {
      let _ = tasks.update_fields(task_id, |t| {
        if t.status != "succeeded" {
          t.status = "cancelled".into();
          t.stage = "cancelled".into();
          t.finished_at = Some(now_secs());
        }
      });
    }
    return Ok(());
  }

  if exit_code != 0 {
    return Err(format!("processor exited {exit_code}"));
  }

  Ok(())
}

fn register_or_report(
  tasks: &TaskStore,
  artifacts: &ArtifactStore,
  task_id: &str,
  out_path: &str,
) -> Result<(), String> {
  if out_path.is_empty() {
    return Ok(());
  }
  let root = PathBuf::from(out_path);
  if !root.join("tileset.json").is_file() {
    return Ok(());
  }
  match artifacts.register(out_path, Some(task_id), "3dtiles", "") {
    Ok(art) => {
      let _ = tasks.update_fields(task_id, |t| {
        let mut prog = t.progress.as_object().cloned().unwrap_or_default();
        prog.insert("artifactId".into(), json!(art.id));
        prog.insert("path".into(), json!(art.path));
        t.progress = Value::Object(prog);
      });
      let _ = tasks.append_log(
        task_id,
        &format!("[processor] registered artifact {}", art.id),
      );
    }
    Err(e) => {
      let _ = tasks.update_fields(task_id, |t| {
        t.status = "succeeded".into();
        let mut prog = t.progress.as_object().cloned().unwrap_or_default();
        prog.insert("registerFailed".into(), json!(true));
        prog.insert("path".into(), json!(out_path));
        t.progress = Value::Object(prog);
        t.error = Some(format!("处理已完成，成果登记失败: {e}"));
      });
      let _ = tasks.append_log(
        task_id,
        &format!("[processor] artifact register failed (files kept): {e}"),
      );
    }
  }
  Ok(())
}

fn apply_event(
  tasks: &TaskStore,
  task_id: &str,
  ev: &Value,
  result_path: &mut Option<String>,
) {
  let ty = ev.get("type").and_then(|v| v.as_str()).unwrap_or("");
  match ty {
    "stage" => {
      let stage = ev
        .get("stage")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
      let message = ev
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
      let _ = tasks.update_fields(task_id, |t| {
        if !stage.is_empty() {
          t.stage = if stage.starts_with("rebuild") {
            "rebuild".into()
          } else if stage == "validate" {
            "check".into()
          } else {
            stage.clone()
          };
        }
        let mut prog = t.progress.as_object().cloned().unwrap_or_default();
        prog.insert("stage".into(), json!(stage));
        if !message.is_empty() {
          prog.insert("message".into(), json!(message));
        }
        if let Some(geo) = ev.get("geo") {
          prog.insert("geo".into(), geo.clone());
        }
        t.progress = Value::Object(prog);
      });
      if !message.is_empty() {
        let _ = tasks.append_log(task_id, &format!("[{stage}] {message}"));
      }
    }
    "progress" => {
      let _ = tasks.update_fields(task_id, |t| {
        let mut prog = t.progress.as_object().cloned().unwrap_or_default();
        if let Some(c) = ev.get("completed") {
          prog.insert("completed".into(), c.clone());
        }
        if let Some(tot) = ev.get("total") {
          prog.insert("total".into(), tot.clone());
        }
        if let Some(st) = ev.get("stage") {
          prog.insert("stage".into(), st.clone());
        }
        t.progress = Value::Object(prog);
      });
    }
    "log" => {
      if let Some(msg) = ev.get("message").and_then(|v| v.as_str()) {
        let _ = tasks.append_log(task_id, msg);
      }
    }
    "warning" => {
      let code = ev.get("code").and_then(|v| v.as_str()).unwrap_or("WARN");
      let msg = ev.get("message").and_then(|v| v.as_str()).unwrap_or("");
      let _ = tasks.append_log(task_id, &format!("[warning:{code}] {msg}"));
    }
    "error" => {
      let code = ev.get("code").and_then(|v| v.as_str()).unwrap_or("ERROR");
      let msg = ev.get("message").and_then(|v| v.as_str()).unwrap_or("");
      let _ = tasks.append_log(task_id, &format!("[error:{code}] {msg}"));
      let _ = tasks.update_fields(task_id, |t| {
        if code != "CANCELLED" {
          if t.error.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
            t.error = Some(msg.to_string());
          }
        }
      });
    }
    "result" => {
      if let Some(p) = ev.get("path").and_then(|v| v.as_str()) {
        *result_path = Some(p.to_string());
        let _ = tasks.append_log(task_id, &format!("[result] {p}"));
      }
    }
    "metric" => {
      let name = ev.get("name").and_then(|v| v.as_str()).unwrap_or("metric");
      let val = ev.get("value").cloned().unwrap_or(Value::Null);
      let _ = tasks.append_log(task_id, &format!("[metric] {name}={val}"));
    }
    _ => {
      let _ = tasks.append_log(task_id, &format!("[event] {ev}"));
    }
  }
}
