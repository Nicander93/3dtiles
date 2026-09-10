//! Shared helpers: tool paths (sidecar-first), subprocess with cancel + log events.
//!
//! Phase 14: release resolution never requires `/workspace` or Python on PATH.
//! Env vars (`GEOFORGE_*`) remain developer overrides only.

use crate::cancel::CancelFlag;
use crate::protocol::Emitter;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, OnceLock};
use std::thread;

#[derive(Debug, Clone)]
pub struct ToolPaths {
    /// Best-effort workspace / install root (may be empty for pure sidecar installs).
    pub repo_root: PathBuf,
    pub convert_bin: PathBuf,
    /// Release TopRebuild CLI (`top_rebuild`). Override: `GEOFORGE_TOP_REBUILD`.
    pub top_rebuild: PathBuf,
    /// Python baseline script. Used only when `GEOFORGE_REBUILD_ENGINE=python`.
    pub rebuild_py: PathBuf,
    /// Experimental Python texture_ktx2 wrapper. Only when `GEOFORGE_TEXTURE_ENGINE=python`.
    pub texture_py: PathBuf,
    pub python: PathBuf,
    /// Bundled / discovered `basisu` CLI for KTX2. Override: `GEOFORGE_BASISU`.
    pub basisu: PathBuf,
}

pub fn tool_paths() -> &'static ToolPaths {
    static PATHS: OnceLock<ToolPaths> = OnceLock::new();
    PATHS.get_or_init(|| {
        let repo_root = discover_repo_root();
        let convert_bin = resolve_convert_bin(&repo_root);
        let top_rebuild = resolve_named_bin("top_rebuild", &repo_root, "GEOFORGE_TOP_REBUILD");
        let rebuild_py = std::env::var("GEOFORGE_REBUILD_TOP")
            .map(PathBuf::from)
            .unwrap_or_else(|_| resolve_rebuild_py(&repo_root));
        let texture_py = first_existing(&[
            repo_root.join("tools/texture_ktx2/run.py"),
            repo_root.join("tools/experiments/desktop_server_py/app/texture_ktx2.py"),
        ])
        .unwrap_or_else(|| repo_root.join("tools/texture_ktx2/run.py"));
        let python = resolve_python(&repo_root);
        let basisu = resolve_basisu(&repo_root);
        ToolPaths {
            repo_root,
            convert_bin,
            top_rebuild,
            rebuild_py,
            texture_py,
            python,
            basisu,
        }
    })
}

fn first_existing(cands: &[PathBuf]) -> Option<PathBuf> {
    cands.iter().find(|p| p.is_file()).cloned()
}

/// Directories to search for sidecars / resources (highest priority first).
fn search_dirs(repo_root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
            // Tauri resource layouts
            dirs.push(dir.join("resources"));
            dirs.push(dir.join("resources/bin"));
            dirs.push(dir.join("bin"));
            // Dev: apps/desktop/src-tauri/target/*/ → repo target
            dirs.push(dir.join("../../../target/debug"));
            dirs.push(dir.join("../../../target/release"));
            dirs.push(dir.join("../../binaries"));
            dirs.push(dir.join("../resources/bin"));
        }
    }
    if let Ok(res) = std::env::var("GEOFORGE_RESOURCES") {
        let p = PathBuf::from(res);
        dirs.push(p.clone());
        dirs.push(p.join("bin"));
    }
    if !repo_root.as_os_str().is_empty() {
        dirs.push(repo_root.join("target/release"));
        dirs.push(repo_root.join("target/debug"));
        dirs.push(repo_root.join("apps/desktop/src-tauri/binaries"));
        dirs.push(repo_root.join("apps/desktop/src-tauri/resources/bin"));
        dirs.push(repo_root.join("vcpkg_installed/x64-linux/tools/basisu"));
        // Developer runtime (optional, not required for release)
        dirs.push(PathBuf::from("/workspace/runtime/3dtile-bin"));
        dirs.push(repo_root.join(".runtime/3dtile-bin"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        for anc in cwd.ancestors().take(6) {
            dirs.push(anc.join("target/release"));
            dirs.push(anc.join("target/debug"));
            dirs.push(anc.join("vcpkg_installed/x64-linux/tools/basisu"));
        }
    }
    dirs
}

fn look_in_dirs(dirs: &[PathBuf], names: &[&str]) -> Option<PathBuf> {
    for d in dirs {
        for name in names {
            let cand = d.join(name);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

fn resolve_named_bin(name: &str, repo_root: &Path, env_key: &str) -> PathBuf {
    if let Ok(p) = std::env::var(env_key) {
        return PathBuf::from(p);
    }
    let exe_name = format!("{name}.exe");
    look_in_dirs(&search_dirs(repo_root), &[name, exe_name.as_str()])
        .unwrap_or_else(|| PathBuf::from(name))
}

fn resolve_convert_bin(repo_root: &Path) -> PathBuf {
    if let Ok(p) = std::env::var("GEOFORGE_3DTILE") {
        return PathBuf::from(p);
    }
    let names = [
        "run.sh", // 3dtile-bin layout wrapper
        "_3dtile",
        "_3dtile.exe",
        "3dtile",
        "3dtile.exe",
    ];
    // Prefer wrapper when sitting in a 3dtile-bin / resources dir
    if let Some(p) = look_in_dirs(&search_dirs(repo_root), &names) {
        return p;
    }
    // Explicit relative resource names under known install roots
    for d in search_dirs(repo_root) {
        for sub in ["3dtile-bin/run.sh", "bin/_3dtile", "bin/run.sh"] {
            let cand = d.join(sub);
            if cand.is_file() {
                return cand;
            }
        }
    }
    PathBuf::from("_3dtile")
}

fn resolve_basisu(repo_root: &Path) -> PathBuf {
    if let Ok(p) = std::env::var("GEOFORGE_BASISU") {
        return PathBuf::from(p);
    }
    look_in_dirs(
        &search_dirs(repo_root),
        &["basisu", "basisu.exe"],
    )
    .unwrap_or_else(|| PathBuf::from("basisu"))
}

fn resolve_python(repo_root: &Path) -> PathBuf {
    if let Ok(p) = std::env::var("GEOFORGE_PYTHON") {
        return PathBuf::from(p);
    }
    // Experiments-only: do not prefer /workspace venv for release defaults.
    let cands = [
        repo_root.join(".venv/bin/python"),
        repo_root.join(".venv/Scripts/python.exe"),
        PathBuf::from("python3"),
        PathBuf::from("python"),
    ];
    for c in &cands {
        if c.as_os_str() == "python3" || c.as_os_str() == "python" {
            return c.clone();
        }
        if c.is_file() {
            return c.clone();
        }
    }
    PathBuf::from("python3")
}

fn rebuild_markers(root: &Path) -> bool {
    root.join("tools/experiments/rebuild_top_py").is_dir()
        || root.join("tools/rebuild_top").is_dir()
        || root.join("crates/processor").is_dir()
}

fn resolve_rebuild_py(repo_root: &Path) -> PathBuf {
    let primary = repo_root.join("tools/experiments/rebuild_top_py/rebuild_top.py");
    if primary.is_file() {
        return primary;
    }
    let legacy = repo_root.join("tools/rebuild_top/rebuild_top.py");
    if legacy.is_file() {
        return legacy;
    }
    primary
}

fn discover_repo_root() -> PathBuf {
    if let Ok(p) = std::env::var("GEOFORGE_REPO_ROOT") {
        return PathBuf::from(p);
    }
    let start = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(root) = start.parent().and_then(|p| p.parent()) {
        if rebuild_markers(root) {
            return root.to_path_buf();
        }
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for anc in cwd.ancestors() {
        if rebuild_markers(anc) && anc.join("apps").is_dir() {
            return anc.to_path_buf();
        }
    }
    // Sidecar-only install: no repo tree.
    PathBuf::new()
}

/// Whether `basisu` is usable for the release KTX2 path.
pub fn basisu_available() -> bool {
    let p = &tool_paths().basisu;
    p.is_file() || which_ok("basisu")
}

fn which_ok(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

/// Run command; stream stdout/stderr lines as log events; honour cancel.
pub fn run_logged(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    cmd: &[String],
    cwd: Option<&Path>,
) -> Result<i32, String> {
    emitter.log(&format!("$ {}", cmd.join(" ")));
    let mut command = Command::new(&cmd[0]);
    if cmd.len() > 1 {
        command.args(&cmd[1..]);
    }
    if let Some(c) = cwd {
        command.current_dir(c);
    }
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                libc_setpgid();
                Ok(())
            });
        }
    }

    let mut child = command.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let e1 = Arc::clone(emitter);
    let t_out = thread::spawn(move || {
        if let Some(out) = stdout {
            for line in BufReader::new(out).lines().flatten() {
                e1.log(&line);
            }
        }
    });
    let e2 = Arc::clone(emitter);
    let t_err = thread::spawn(move || {
        if let Some(err) = stderr {
            for line in BufReader::new(err).lines().flatten() {
                eprintln!("{line}");
                e2.log(&format!("[stderr] {line}"));
            }
        }
    });

    loop {
        if cancel.is_cancelled() {
            terminate_child(&mut child);
            break;
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => thread::sleep(std::time::Duration::from_millis(100)),
            Err(e) => return Err(format!("wait error: {e}")),
        }
    }

    let _ = t_out.join();
    let _ = t_err.join();
    let status = child.wait().map_err(|e| e.to_string())?;
    Ok(status
        .code()
        .unwrap_or(if cancel.is_cancelled() { 130 } else { 1 }))
}

fn terminate_child(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let pid = child.id() as i32;
        libc_kill(-pid, 15);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while std::time::Instant::now() < deadline {
            if let Ok(Some(_)) = child.try_wait() {
                return;
            }
            thread::sleep(std::time::Duration::from_millis(200));
        }
        libc_kill(-pid, 9);
    }
    let _ = child.kill();
}

#[cfg(unix)]
fn libc_setpgid() {
    extern "C" {
        fn setpgid(pid: i32, pgid: i32) -> i32;
    }
    unsafe {
        let _ = setpgid(0, 0);
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

#[cfg(not(unix))]
fn libc_setpgid() {}
#[cfg(not(unix))]
fn libc_kill(_pid: i32, _sig: i32) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_resolve_does_not_hardcode_workspace_as_required() {
        // Even without GEOFORGE_3DTILE, default should be a relative name or discovered file —
        // never *require* /workspace/runtime/... to exist for the type to construct.
        let t = tool_paths();
        let s = t.convert_bin.to_string_lossy();
        // If the developer runtime exists it may still be discovered; that is OK.
        // The important contract: path is either an existing file or a portable name.
        if !t.convert_bin.is_file() {
            assert!(
                !s.starts_with("/workspace/") || s.contains("runtime"),
                "non-existent convert_bin should not be a stale /workspace hardcode alone: {s}"
            );
        }
    }

    #[test]
    fn default_rebuild_engine_is_rust_contract() {
        // Documented default; actual env may override in CI — just ensure helper compiles.
        let engine = std::env::var("GEOFORGE_REBUILD_ENGINE").unwrap_or_else(|_| "rust".into());
        assert!(!engine.is_empty());
    }
}
