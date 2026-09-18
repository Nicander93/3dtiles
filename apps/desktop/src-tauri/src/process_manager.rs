//! Spawn `processor` child, Job Object / process-group cancel, serial queue (T03/T04).

use crate::artifact_store::ArtifactStore;
use crate::db::now_secs;
use crate::task_store::TaskStore;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Default grace period after cooperative cancel before force-killing the process tree.
pub const CANCEL_GRACE_SECS: u64 = 10;
const MAX_DIAGNOSTIC_DISPLAY_BYTES: usize = 64 * 1024;
const MAX_PROTOCOL_LINE_BYTES: usize = 64 * 1024;

#[derive(Clone, Default)]
pub struct ProcessManager {
  inner: Arc<Mutex<HashMap<String, ActiveProc>>>,
  /// Single-flight serial runner.
  scheduler: Arc<Mutex<Scheduler>>,
  /// Set when the desktop is closing so the scheduler cannot start another task.
  stopping: Arc<AtomicBool>,
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

#[cfg(test)]
mod tests {
  use super::{
    apply_event, copy_raw_stream, read_bounded_protocol_line, MAX_DIAGNOSTIC_DISPLAY_BYTES,
  };
  use crate::db::init_schema;
  use crate::task_store::TaskStore;
  use parking_lot::Mutex;
  use rusqlite::Connection;
  use serde_json::json;
  use std::io::BufReader;
  use std::sync::Arc;

  #[test]
  fn raw_stream_preserves_bytes_while_bounding_display_fragments() {
    let mut input = vec![b'x'; MAX_DIAGNOSTIC_DISPLAY_BYTES * 2 + 9];
    input.extend_from_slice(b"\nlast\n");
    let mut raw = Vec::new();
    let mut fragments = Vec::new();
    copy_raw_stream(input.as_slice(), &mut raw, |bytes, truncated| {
      fragments.push((bytes.len(), truncated));
    })
    .expect("copy diagnostic stream");

    assert_eq!(raw, input);
    assert!(fragments
      .iter()
      .all(|(length, _)| *length <= MAX_DIAGNOSTIC_DISPLAY_BYTES));
    assert!(fragments.iter().any(|(_, truncated)| *truncated));
  }

  #[test]
  fn protocol_reader_preserves_raw_bytes_while_bounding_one_line() {
    let mut input = vec![b'{'; super::MAX_PROTOCOL_LINE_BYTES + 17];
    input.push(b'\n');
    let mut reader = BufReader::new(input.as_slice());
    let mut raw = Vec::new();
    let (display, truncated) = read_bounded_protocol_line(&mut reader, &mut raw)
      .expect("read protocol line")
      .expect("line exists");
    assert_eq!(raw, input);
    assert_eq!(display.len(), super::MAX_PROTOCOL_LINE_BYTES);
    assert!(truncated);
  }

  #[test]
  fn error_event_preserves_code_stage_message_and_log() {
    let conn = Connection::open_in_memory().expect("open sqlite");
    init_schema(&conn).expect("create schema");
    let data_dir = std::env::temp_dir().join(format!(
      "geoforge-process-error-event-{}",
      std::process::id()
    ));
    let tasks = TaskStore::new(Arc::new(Mutex::new(conn)), data_dir.clone());
    let task = tasks
      .create("convert-osgb", "input", "output", json!({}), "error-event")
      .expect("create task");
    tasks
      .update_fields(&task.id, |saved| saved.stage = "convert".into())
      .expect("set stage");

    let mut result_path = None;
    apply_event(
      &tasks,
      &task.id,
      &json!({
        "type": "error",
        "code": "CONVERTER_EXIT_NONZERO",
        "message": "Tile_甲.osgb failed: access denied"
      }),
      &mut result_path,
    );

    let saved = tasks
      .get(&task.id)
      .expect("read task")
      .expect("task exists");
    assert_eq!(saved.error.as_deref(), Some("Tile_甲.osgb failed: access denied"));
    assert_eq!(saved.progress["errorCode"], "CONVERTER_EXIT_NONZERO");
    assert_eq!(saved.progress["failedStage"], "convert");
    assert!(saved.log.contains("[error:CONVERTER_EXIT_NONZERO]"));
    assert!(saved.log.contains("Tile_甲.osgb failed"));
    let _ = std::fs::remove_dir_all(data_dir);
  }
}

#[cfg(all(test, windows))]
mod windows_job_tests {
  use super::{attach_job_object, ActiveProc, ProcessManager};
  use crate::db::init_schema;
  use crate::task_store::TaskStore;
  use parking_lot::Mutex;
  use rusqlite::Connection;
  use std::process::Command;
  use std::sync::atomic::AtomicBool;
  use std::sync::Arc;
  use std::thread;
  use std::time::{Duration, Instant};

  #[test]
  fn kill_on_close_terminates_attached_processor_child() {
    let started = Instant::now();
    let mut child = Command::new("cmd.exe")
      .args(["/C", "ping.exe 127.0.0.1 -n 30 > NUL"])
      .spawn()
      .expect("spawn Windows child for Job Object smoke test");
    let (job, error) = attach_job_object(&child);
    assert!(error.is_none(), "Job Object setup failed: {error:?}");
    drop(job);

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
      if let Some(status) = child.try_wait().expect("poll child after job close") {
        let elapsed = started.elapsed();
        assert!(
          elapsed < Duration::from_secs(3),
          "child exited too slowly after Job Object close: {elapsed:?} (status {status:?})"
        );
        break;
      }
      assert!(Instant::now() < deadline, "Job Object did not terminate child");
      thread::sleep(Duration::from_millis(25));
    }
  }

  #[test]
  fn shutdown_marks_task_interrupted_and_kills_processor_child() {
    let conn = Connection::open_in_memory().expect("open sqlite");
    init_schema(&conn).expect("create schema");
    let data_dir = std::env::temp_dir().join(format!(
      "geoforge-shutdown-test-{}",
      std::process::id()
    ));
    let tasks = TaskStore::new(Arc::new(Mutex::new(conn)), data_dir.clone());
    let task = tasks
      .create("convert-osgb", "input", "output", serde_json::json!({}), "test")
      .expect("create task");

    let child = Command::new("cmd.exe")
      .args(["/C", "ping.exe 127.0.0.1 -n 30 > NUL"])
      .spawn()
      .expect("spawn Windows shutdown child");
    let (job, error) = attach_job_object(&child);
    assert!(error.is_none(), "Job Object setup failed: {error:?}");
    let manager = ProcessManager::new();
    manager.inner.lock().insert(
      task.id.clone(),
      ActiveProc {
        child,
        stdin: None,
        job,
        force_kill: Arc::new(AtomicBool::new(false)),
        committed: Arc::new(AtomicBool::new(false)),
      },
    );

    manager.shutdown(&tasks);

    let saved = tasks.get(&task.id).expect("read task").expect("task exists");
    assert_eq!(saved.status, "interrupted");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
      let exited = manager
        .inner
        .lock()
        .get_mut(&task.id)
        .and_then(|proc| proc.child.try_wait().ok().flatten())
        .is_some();
      if exited {
        break;
      }
      assert!(Instant::now() < deadline, "shutdown child did not exit");
      thread::sleep(Duration::from_millis(25));
    }
    manager.inner.lock().remove(&task.id);
    let _ = std::fs::remove_dir_all(data_dir);
  }
}

#[cfg(windows)]
#[repr(C)]
struct JobObjectBasicLimitInformation {
  per_process_user_time_limit: i64,
  per_job_user_time_limit: i64,
  limit_flags: u32,
  minimum_working_set_size: usize,
  maximum_working_set_size: usize,
  active_process_limit: u32,
  affinity: usize,
  priority_class: u32,
  scheduling_class: u32,
}

#[cfg(windows)]
#[repr(C)]
struct IoCounters {
  read_operations: u64,
  write_operations: u64,
  other_operations: u64,
  read_bytes: u64,
  write_bytes: u64,
  other_bytes: u64,
}

#[cfg(windows)]
#[repr(C)]
struct JobObjectExtendedLimitInformation {
  basic_limit_information: JobObjectBasicLimitInformation,
  io_info: IoCounters,
  process_memory_limit: usize,
  job_memory_limit: usize,
  peak_process_memory_used: usize,
  peak_job_memory_used: usize,
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
    processor::util::hide_console_window(command);
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
        if mgr.stopping.load(Ordering::SeqCst) || rx.recv().is_err() {
          break;
        }
        if mgr.stopping.load(Ordering::SeqCst) {
          break;
        }
        // Drain extra kicks
        while rx.try_recv().is_ok() {}
        loop {
          if mgr.stopping.load(Ordering::SeqCst) {
            break;
          }
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
            let error_code = if e.contains("exited") {
              "PROCESSOR_EXIT_NONZERO"
            } else {
              "PROCESSOR_FAILED"
            };
            let _ = tasks.append_log(&tid, &format!("[processor] error: {e}"));
            let _ = tasks.update_fields(&tid, |t| {
              if mgr.stopping.load(Ordering::SeqCst) {
                return;
              }
              if t.status != "cancelled" && t.status != "cancelling" {
                // Keep first useful error if already set
                if t.error.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
                  t.error = Some(e);
                } else if !e.contains("exited") {
                  // prefer more specific existing error; append exit note
                  let prev = t.error.clone().unwrap_or_default();
                  t.error = Some(format!("{prev}; {e}"));
                }
                let mut prog = t.progress.as_object().cloned().unwrap_or_default();
                prog.insert("errorCode".into(), json!(error_code));
                prog.insert("failedStage".into(), json!(t.stage.clone()));
                t.progress = Value::Object(prog);
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
        #[allow(unused_unsafe)]
        unsafe {
          libc_kill(-(pid as i32), 15);
        }
        thread::sleep(Duration::from_secs(2));
        #[allow(unused_unsafe)]
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

  /// Stop scheduling and terminate every processor owned by this desktop.
  ///
  /// Tasks are marked interrupted before termination so the next launch can
  /// offer a clear recovery state. The scheduler observes `stopping` and
  /// exits after the current processor task unwinds.
  pub fn shutdown(&self, tasks: &TaskStore) {
    self.stopping.store(true, Ordering::SeqCst);
    let _ = tasks.mark_stale_interrupted();

    let mut map = self.inner.lock();
    for proc in map.values_mut() {
      proc.force_kill.store(true, Ordering::SeqCst);
      terminate_active_process(proc);
    }
    drop(map);
    // Wake a scheduler blocked in recv so it can observe stopping.
    self.kick();
  }
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
  fn CreateJobObjectW(
    lpJobAttributes: *mut std::ffi::c_void,
    lpName: *const u16,
  ) -> *mut std::ffi::c_void;
  fn SetInformationJobObject(
    hJob: *mut std::ffi::c_void,
    job_object_information_class: u32,
    job_object_information: *mut std::ffi::c_void,
    job_object_information_length: u32,
  ) -> i32;
  fn AssignProcessToJobObject(hJob: *mut std::ffi::c_void, hProcess: *mut std::ffi::c_void) -> i32;
  fn TerminateJobObject(hJob: *mut std::ffi::c_void, uExitCode: u32) -> i32;
  fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
}

#[cfg(windows)]
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: u32 = 9;
#[cfg(windows)]
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

#[cfg(windows)]
fn attach_job_object(child: &Child) -> (Option<JobHandle>, Option<&'static str>) {
  use std::os::windows::io::AsRawHandle;

  unsafe {
    let h = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
    if h.is_null() {
      return (
        None,
        Some("CreateJobObjectW failed; process tree is not job-controlled"),
      );
    }
    let mut limits = JobObjectExtendedLimitInformation {
      basic_limit_information: JobObjectBasicLimitInformation {
        per_process_user_time_limit: 0,
        per_job_user_time_limit: 0,
        limit_flags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        minimum_working_set_size: 0,
        maximum_working_set_size: 0,
        active_process_limit: 0,
        affinity: 0,
        priority_class: 0,
        scheduling_class: 0,
      },
      io_info: IoCounters {
        read_operations: 0,
        write_operations: 0,
        other_operations: 0,
        read_bytes: 0,
        write_bytes: 0,
        other_bytes: 0,
      },
      process_memory_limit: 0,
      job_memory_limit: 0,
      peak_process_memory_used: 0,
      peak_job_memory_used: 0,
    };
    let configured = SetInformationJobObject(
      h,
      JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
      (&mut limits as *mut JobObjectExtendedLimitInformation).cast(),
      std::mem::size_of::<JobObjectExtendedLimitInformation>() as u32,
    );
    if configured == 0 {
      let _ = CloseHandle(h);
      (
        None,
        Some("SetInformationJobObject failed; process tree is not job-controlled"),
      )
    } else if AssignProcessToJobObject(h, child.as_raw_handle() as *mut _) == 0 {
      let _ = CloseHandle(h);
      (
        None,
        Some("AssignProcessToJobObject failed; process tree is not job-controlled"),
      )
    } else {
      (Some(JobHandle(h)), None)
    }
  }
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

/// A broken protocol/diagnostic pipe must not leave the processor running
/// after the scheduler has observed the task failure.
fn force_terminate_process(mgr: &ProcessManager, task_id: &str) {
  let mut map = mgr.inner.lock();
  let Some(proc) = map.get_mut(task_id) else {
    return;
  };
  proc.force_kill.store(true, Ordering::SeqCst);
  terminate_active_process(proc);
}

fn terminate_active_process(proc: &mut ActiveProc) {
  #[cfg(windows)]
  {
    if let Some(job) = proc.job.as_ref() {
      unsafe {
        let _ = TerminateJobObject(job.0, 1);
      }
    } else {
      let _ = proc.child.kill();
    }
  }
  #[cfg(unix)]
  {
    let pid = proc.child.id() as i32;
    libc_kill(-pid, 9);
    let _ = proc.child.kill();
  }
  #[cfg(not(any(windows, unix)))]
  {
    let _ = proc.child.kill();
  }
}

/// Copy raw bytes to a diagnostic file while delivering bounded display
/// fragments to the task log. A converter that never emits newlines therefore
/// cannot grow one in-memory line indefinitely.
fn copy_raw_stream<R: Read, W: Write, F: FnMut(Vec<u8>, bool)>(
  mut reader: R,
  mut raw_file: W,
  mut on_fragment: F,
) -> std::io::Result<()> {
  let mut pending = Vec::new();
  let mut buffer = [0_u8; 8192];
  loop {
    let count = reader.read(&mut buffer)?;
    if count == 0 {
      break;
    }
    raw_file.write_all(&buffer[..count])?;
    pending.extend_from_slice(&buffer[..count]);
    loop {
      let Some(newline) = pending.iter().position(|byte| *byte == b'\n') else {
        break;
      };
      let rest = pending.split_off(newline + 1);
      let line = std::mem::replace(&mut pending, rest);
      emit_bounded_fragment(line, &mut on_fragment);
    }
    while pending.len() > MAX_DIAGNOSTIC_DISPLAY_BYTES {
      let chunk = pending.drain(..MAX_DIAGNOSTIC_DISPLAY_BYTES).collect();
      on_fragment(chunk, true);
    }
  }
  if !pending.is_empty() {
    emit_bounded_fragment(pending, &mut on_fragment);
  }
  Ok(())
}

fn emit_bounded_fragment<F: FnMut(Vec<u8>, bool)>(mut bytes: Vec<u8>, on_fragment: &mut F) {
  while bytes.len() > MAX_DIAGNOSTIC_DISPLAY_BYTES {
    let chunk = bytes.drain(..MAX_DIAGNOSTIC_DISPLAY_BYTES).collect();
    on_fragment(chunk, true);
  }
  if !bytes.is_empty() {
    on_fragment(bytes, false);
  }
}

/// Read one processor JSONL record without allowing an unterminated record
/// to grow without bound. The raw bytes are written in full for diagnostics;
/// only the bounded prefix is returned for protocol parsing/display.
fn read_bounded_protocol_line<R: BufRead, W: Write>(
  reader: &mut R,
  raw_file: &mut W,
) -> std::io::Result<Option<(Vec<u8>, bool)>> {
  let mut display = Vec::new();
  let mut truncated = false;
  loop {
    let chunk = reader.fill_buf()?;
    if chunk.is_empty() {
      if display.is_empty() && !truncated {
        return Ok(None);
      }
      return Ok(Some((display, truncated)));
    }
    let line_end = chunk.iter().position(|byte| *byte == b'\n');
    let consume = line_end.map(|index| index + 1).unwrap_or(chunk.len());
    raw_file.write_all(&chunk[..consume])?;
    if display.len() < MAX_PROTOCOL_LINE_BYTES {
      let take = (MAX_PROTOCOL_LINE_BYTES - display.len()).min(consume);
      display.extend_from_slice(&chunk[..take]);
      if take < consume {
        truncated = true;
      }
    } else if consume > 0 {
      truncated = true;
    }
    reader.consume(consume);
    if line_end.is_some() {
      return Ok(Some((display, truncated)));
    }
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
  if mgr.stopping.load(Ordering::SeqCst) {
    return Ok(());
  }
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

  let bin = resolve_processor_bin()
    .ok_or_else(|| "找不到 processor 组件。请修复安装或设置 GEOFORGE_PROCESSOR。".to_string())?;

  let _ = tasks.append_log(task_id, &format!("[processor] spawning {}", bin.display()));

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

  // Keep protocol stdout separate from raw diagnostics. The JSONL stream is
  // still consumed below, while these sidecars preserve bytes that cannot be
  // decoded for display.
  let mut stdout_log_path = tasks.log_path_for(task_id);
  stdout_log_path.set_extension("stdout.jsonl");
  let mut stderr_log_path = tasks.log_path_for(task_id);
  stderr_log_path.set_extension("stderr.log");
  if let Some(parent) = stdout_log_path.parent() {
    std::fs::create_dir_all(parent)
      .map_err(|e| format!("create diagnostics directory failed: {e}"))?;
  }
  std::fs::File::create(&stdout_log_path)
    .map_err(|e| format!("create processor stdout diagnostics failed: {e}"))?;
  std::fs::File::create(&stderr_log_path)
    .map_err(|e| format!("create processor stderr diagnostics failed: {e}"))?;

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
  let (job, job_error) = attach_job_object(&child);
  #[cfg(windows)]
  if let Some(error) = job_error {
    let _ = tasks.append_log(task_id, &format!("[desktop:windows] {error}"));
  }

  let pid = child.id() as i64;
  let stdin = child.stdin.take();
  let stdout = child.stdout.take().ok_or_else(|| "no stdout".to_string())?;
  let stderr = child.stderr.take();
  let committed = Arc::new(AtomicBool::new(false));
  let force_kill = Arc::new(AtomicBool::new(false));

  if let Some(err) = stderr {
    let tasks_c = tasks.clone();
    let tid = task_id.to_string();
    let raw_path = stderr_log_path.clone();
    thread::spawn(move || {
      let mut raw_file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&raw_path)
      {
        Ok(file) => file,
        Err(error) => {
          let _ = tasks_c.append_log(
            &tid,
            &format!("[processor:stderr] cannot open raw diagnostics: {error}"),
          );
          return;
        }
      };
      let result = copy_raw_stream(err, &mut raw_file, |bytes, truncated| {
        let mut line = String::from_utf8_lossy(&bytes)
          .trim_end_matches(['\r', '\n'])
          .to_string();
        if truncated {
          line.push_str(" …<line truncated>");
        }
        let _ = tasks_c.append_log(&tid, &format!("[processor:stderr] {line}"));
      });
      if let Err(error) = result {
        let _ = tasks_c.append_log(&tid, &format!("[processor:stderr] read failed: {error}"));
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

  // A close request can race with process creation. Re-check after the
  // process is visible in the manager so the shutdown path cannot miss it.
  if mgr.stopping.load(Ordering::SeqCst) {
    force_terminate_process(mgr, task_id);
  }

  let _ = tasks.update_fields(task_id, |t| {
    if mgr.stopping.load(Ordering::SeqCst) {
      return;
    }
    t.status = "running".into();
    t.stage = "scan".into();
    t.started_at = Some(now_secs());
    t.pid = Some(pid);
    let mut prog = t.progress.as_object().cloned().unwrap_or_default();
    prog.insert("executor".into(), json!("processor"));
    prog.insert(
      "stdoutLogPath".into(),
      json!(stdout_log_path.to_string_lossy().into_owned()),
    );
    prog.insert(
      "stderrLogPath".into(),
      json!(stderr_log_path.to_string_lossy().into_owned()),
    );
    t.progress = Value::Object(prog);
  });

  let mut result_path: Option<String> = None;
  let mut reader = BufReader::new(stdout);
  let mut raw_stdout = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(&stdout_log_path)
    .map_err(|e| {
      force_terminate_process(mgr, task_id);
      format!("open processor stdout diagnostics failed: {e}")
    })?;
  loop {
    let Some((bytes, truncated)) = read_bounded_protocol_line(&mut reader, &mut raw_stdout)
      .map_err(|e| {
      force_terminate_process(mgr, task_id);
      format!("read processor stdout failed: {e}")
    })?
    else {
      break;
    };
    let line = String::from_utf8_lossy(&bytes).trim().to_string();
    if line.is_empty() {
      continue;
    }
    if let Ok(Some(t)) = tasks.get(task_id) {
      if t.cancel_requested && !committed.load(Ordering::SeqCst) {
        mgr.cancel(task_id);
      }
    }
    if truncated {
      let _ = tasks.append_log(
        task_id,
        &format!(
          "[processor:raw] protocol line exceeded {} bytes; see stdout diagnostics",
          MAX_PROTOCOL_LINE_BYTES
        ),
      );
      continue;
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

  let task_now = tasks
    .get(task_id)?
    .ok_or_else(|| "task missing".to_string())?;

  // Shutdown already persisted `interrupted`; do not rewrite it as success,
  // cancellation, or a generic processor failure while the child unwinds.
  if mgr.stopping.load(Ordering::SeqCst) {
    return Ok(());
  }

  // Success + result: never rewrite to cancelled
  if exit_code == 0 && (result_path.is_some() || committed.load(Ordering::SeqCst)) {
    let out_path = result_path
      .clone()
      .unwrap_or_else(|| task_now.output.path.clone());
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

fn apply_event(tasks: &TaskStore, task_id: &str, ev: &Value, result_path: &mut Option<String>) {
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
        let mut prog = t.progress.as_object().cloned().unwrap_or_default();
        prog.insert("errorCode".into(), json!(code));
        prog.insert("errorMessage".into(), json!(msg));
        prog.insert("failedStage".into(), json!(t.stage.clone()));
        t.progress = Value::Object(prog);
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
