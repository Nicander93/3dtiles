//! Shared helpers: tool paths, subprocess with cancel + log events.

use crate::cancel::CancelFlag;
use crate::protocol::Emitter;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, OnceLock};
use std::thread;

#[derive(Debug, Clone)]
pub struct ToolPaths {
    pub repo_root: PathBuf,
    pub runtime_root: PathBuf,
    pub convert_bin: PathBuf,
    /// Release TopRebuild CLI (`top_rebuild`). Override: `GEOFORGE_TOP_REBUILD`.
    pub top_rebuild: PathBuf,
    /// Python baseline script. Used only when `GEOFORGE_REBUILD_ENGINE=python`.
    pub rebuild_py: PathBuf,
    pub texture_py: PathBuf,
    /// Packaged texture tool (geoforge-texture.exe). Override: `GEOFORGE_TEXTURE`.
    pub texture_bin: PathBuf,
    pub basisu: PathBuf,
    pub python: PathBuf,
    pub packaged: bool,
}

pub fn tool_paths() -> &'static ToolPaths {
    static PATHS: OnceLock<ToolPaths> = OnceLock::new();
    PATHS.get_or_init(|| {
        let packaged = std::env::var("GEOFORGE_PACKAGED").ok().as_deref() == Some("1")
            || std::env::var("GEOFORGE_RUNTIME_ROOT").is_ok();
        let repo_root = discover_repo_root();
        let runtime_root = resolve_runtime_root(&repo_root);
        let convert_bin = resolve_3dtile(&repo_root, &runtime_root);
        let top_rebuild = resolve_top_rebuild(&repo_root, &runtime_root);
        let rebuild_py = std::env::var("GEOFORGE_REBUILD_TOP")
            .map(PathBuf::from)
            .unwrap_or_else(|_| resolve_rebuild_py(&repo_root));
        let texture_py = repo_root.join("tools/texture_ktx2/run.py");
        let texture_bin = resolve_texture_bin(&runtime_root);
        let basisu = resolve_basisu(&runtime_root, &repo_root);
        let python = std::env::var("GEOFORGE_PYTHON")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("python3"));
        ToolPaths {
            repo_root,
            runtime_root,
            convert_bin,
            top_rebuild,
            rebuild_py,
            texture_py,
            texture_bin,
            basisu,
            python,
            packaged,
        }
    })
}

fn resolve_runtime_root(repo_root: &Path) -> PathBuf {
    if let Ok(p) = std::env::var("GEOFORGE_RUNTIME_ROOT") {
        return PathBuf::from(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("resources").join("runtime");
            if bundled.is_dir() {
                return bundled;
            }
            // processor next to app; runtime under resources/
            let alt = dir.join("runtime");
            if alt.is_dir() {
                return alt;
            }
        }
    }
    repo_root.join("dist").join("runtime")
}

fn resolve_texture_bin(runtime_root: &Path) -> PathBuf {
    if let Ok(p) = std::env::var("GEOFORGE_TEXTURE") {
        return PathBuf::from(p);
    }
    for name in ["geoforge-texture.exe", "geoforge-texture"] {
        let c = runtime_root.join("texture").join(name);
        if c.is_file() {
            return c;
        }
    }
    PathBuf::from("geoforge-texture")
}

fn resolve_basisu(runtime_root: &Path, repo_root: &Path) -> PathBuf {
    if let Ok(p) = std::env::var("GEOFORGE_BASISU") {
        return PathBuf::from(p);
    }
    for c in [
        runtime_root.join("texture").join("basisu.exe"),
        runtime_root.join("texture").join("basisu"),
        repo_root.join("vcpkg_installed/x64-windows/tools/basisu/basisu.exe"),
    ] {
        if c.is_file() {
            return c;
        }
    }
    PathBuf::from("basisu")
}

fn rebuild_markers(root: &Path) -> bool {
    root.join("tools/experiments/rebuild_top_py").is_dir()
        || root.join("tools/rebuild_top").is_dir()
}

fn resolve_rebuild_py(repo_root: &Path) -> PathBuf {
    let primary = repo_root.join("tools/experiments/rebuild_top_py/rebuild_top.py");
    if primary.is_file() {
        return primary;
    }
    // One-release fallback for checkouts that still have the old path.
    let legacy = repo_root.join("tools/rebuild_top/rebuild_top.py");
    if legacy.is_file() {
        return legacy;
    }
    primary
}

fn first_existing(cands: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    cands.into_iter().find(|p| p.is_file())
}

fn bin_names(stem: &str) -> [String; 2] {
    [stem.to_string(), format!("{stem}.exe")]
}

fn sibling_bins(dir: &Path, stem: &str) -> Vec<PathBuf> {
    bin_names(stem).into_iter().map(|name| dir.join(name)).collect()
}

fn profile_bins(root: &Path, stem: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for profile in ["release", "debug"] {
        for name in bin_names(stem) {
            out.push(root.join("target").join(profile).join(name));
        }
    }
    out
}

/// Order: `GEOFORGE_3DTILE` → runtime/converter → next to processor → PATH.
fn resolve_3dtile(_repo_root: &Path, runtime_root: &Path) -> PathBuf {
    if let Ok(p) = std::env::var("GEOFORGE_3DTILE") {
        return PathBuf::from(p);
    }
    if let Some(p) = first_existing([
        runtime_root.join("converter").join("_3dtile.exe"),
        runtime_root.join("converter").join("_3dtile"),
    ]) {
        return p;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if let Some(p) = first_existing(sibling_bins(dir, "_3dtile")) {
                return p;
            }
        }
    }
    PathBuf::from("_3dtile")
}

/// Order: `GEOFORGE_TOP_REBUILD` → runtime/bin → next to processor → repo target → PATH.
fn resolve_top_rebuild(repo_root: &Path, runtime_root: &Path) -> PathBuf {
    if let Ok(p) = std::env::var("GEOFORGE_TOP_REBUILD") {
        return PathBuf::from(p);
    }
    if let Some(p) = first_existing([
        runtime_root.join("bin").join("top_rebuild.exe"),
        runtime_root.join("bin").join("top_rebuild"),
    ]) {
        return p;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if let Some(p) = first_existing(sibling_bins(dir, "top_rebuild")) {
                return p;
            }
        }
    }
    if let Some(p) = first_existing(profile_bins(repo_root, "top_rebuild")) {
        return p;
    }
    PathBuf::from("top_rebuild")
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
    cwd
}

/// Run command; stream stdout/stderr lines as log events; honour cancel.
pub fn run_logged(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    cmd: &[String],
    cwd: Option<&Path>,
) -> Result<i32, String> {
    run_logged_env(emitter, cancel, cmd, cwd, &[])
}

pub fn run_logged_env(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    cmd: &[String],
    cwd: Option<&Path>,
    extra_env: &[(&str, PathBuf)],
) -> Result<i32, String> {
    emitter.log(&format!("$ {}", cmd.join(" ")));
    let mut command = Command::new(&cmd[0]);
    if cmd.len() > 1 {
        command.args(&cmd[1..]);
    }
    if let Some(c) = cwd {
        command.current_dir(c);
    }
    for (k, v) in extra_env {
        command.env(k, v);
    }
    // `_3dtile`/OSG prints plugin dumps from many threads to stdout; piping that
    // race-crashes on Windows (0xC0000005). Keep stderr only (rustc env_logger).
    command
        .stdout(Stdio::null())
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
#[allow(dead_code)]
fn libc_setpgid() {}
#[cfg(not(unix))]
#[allow(dead_code)]
fn libc_kill(_pid: i32, _sig: i32) {}

#[cfg(test)]
mod tests {
    use super::bin_names;

    #[test]
    fn bin_names_include_exe_suffix() {
        let names = bin_names("_3dtile");
        assert_eq!(names[0], "_3dtile");
        assert_eq!(names[1], "_3dtile.exe");
    }
}
