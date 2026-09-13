//! Task I/O path safety: normalize, reject overlap, no overwrite, safe task ids.

use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ValidatedPaths {
    pub input_root: PathBuf,
    pub output: PathBuf,
    pub task_id: String,
}

pub fn validate_task_id(task_id: &str) -> Result<(), String> {
    if task_id.is_empty() || task_id.len() > 128 {
        return Err("invalid taskId".into());
    }
    if task_id.contains(['/', '\\', '.', ':'])
        || task_id.contains("..")
        || !task_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("invalid taskId (path unsafe): {task_id}"));
    }
    Ok(())
}

/// Resolve data root: directory, or parent of tileset.json / metadata.xml etc.
pub fn resolve_input_root(input: &Path) -> Result<PathBuf, String> {
    if !input.exists() {
        return Err(format!("input does not exist: {}", input.display()));
    }
    let canon = canonicalize_existing(input)?;
    if canon.is_file() {
        let name = canon
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if name == "tileset.json" || name.ends_with(".json") || name == "metadata.xml" {
            return canon
                .parent()
                .map(|p| p.to_path_buf())
                .ok_or_else(|| "input file has no parent directory".into());
        }
        return Err(format!(
            "input must be a directory or tileset.json, got file: {}",
            canon.display()
        ));
    }
    Ok(canon)
}

/// Normalize a path that may not exist yet: canonicalize nearest ancestor + rest.
pub fn normalize_path(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("empty path".into());
    }
    if path.exists() {
        return canonicalize_existing(path);
    }
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    let abs = normalize_dots(&abs);
    let mut cur = abs.as_path();
    let mut missing = Vec::new();
    loop {
        if cur.exists() {
            let base = canonicalize_existing(cur)?;
            let mut out = base;
            for part in missing.into_iter().rev() {
                out.push(part);
            }
            return Ok(out);
        }
        match cur.file_name() {
            Some(name) => {
                missing.push(name.to_os_string());
                cur = cur
                    .parent()
                    .ok_or_else(|| format!("cannot resolve path: {}", path.display()))?;
            }
            None => {
                return Err(format!("cannot resolve path: {}", path.display()));
            }
        }
    }
}

fn normalize_dots(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn canonicalize_existing(path: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(windows)]
fn paths_equal(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())
}

#[cfg(not(windows))]
fn paths_equal(a: &Path, b: &Path) -> bool {
    a == b
}

/// True if `child` is strictly inside `parent` (not equal).
pub fn is_strict_descendant(parent: &Path, child: &Path) -> bool {
    if paths_equal(parent, child) {
        return false;
    }
    let mut cur = child.parent();
    while let Some(p) = cur {
        if paths_equal(p, parent) {
            return true;
        }
        cur = p.parent();
    }
    false
}

pub fn validate_io_paths(
    input: &Path,
    output: &Path,
    task_id: &str,
) -> Result<ValidatedPaths, String> {
    validate_task_id(task_id)?;
    let input_root = resolve_input_root(input)?;
    let output_n = normalize_path(output)?;

    if paths_equal(&input_root, &output_n) {
        return Err("output path must not equal input data root".into());
    }
    if is_strict_descendant(&input_root, &output_n) {
        return Err("output path must not be inside input data root".into());
    }
    if is_strict_descendant(&output_n, &input_root) {
        return Err("output path must not contain the input data root".into());
    }

    // Work dir sibling under output parent must also not overlap oddly — covered by input rules.
    if output_n.exists() {
        return Err(format!(
            "output already exists (V1 does not overwrite): {}",
            output_n.display()
        ));
    }

    Ok(ValidatedPaths {
        input_root,
        output: output_n,
        task_id: task_id.to_string(),
    })
}

/// Rename `src` → `dst` without replacing an existing destination. No copy fallback.
pub fn rename_no_replace(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        return Err(format!(
            "commit refused: output appeared during processing: {}",
            dst.display()
        ));
    }
    #[cfg(unix)]
    {
        rename_noreplace_unix(src, dst)
    }
    #[cfg(windows)]
    {
        // Windows rename fails if destination exists (dirs/files).
        fs::rename(src, dst).map_err(|e| format!("commit rename failed: {e}"))?;
        Ok(())
    }
    #[cfg(all(not(unix), not(windows)))]
    {
        fs::rename(src, dst).map_err(|e| format!("commit rename failed: {e}"))
    }
}

#[cfg(unix)]
fn rename_noreplace_unix(src: &Path, dst: &Path) -> Result<(), String> {
    use std::ffi::CString;
    use std::os::raw::c_char;
    use std::os::unix::ffi::OsStrExt;

    const RENAME_NOREPLACE: u32 = 1;
    const AT_FDCWD: i32 = -100;
    extern "C" {
        fn renameat2(
            olddirfd: i32,
            oldpath: *const c_char,
            newdirfd: i32,
            newpath: *const c_char,
            flags: u32,
        ) -> i32;
    }
    let old = CString::new(src.as_os_str().as_bytes())
        .map_err(|_| "src path contains NUL".to_string())?;
    let new = CString::new(dst.as_os_str().as_bytes())
        .map_err(|_| "dst path contains NUL".to_string())?;
    let rc = unsafe { renameat2(AT_FDCWD, old.as_ptr(), AT_FDCWD, new.as_ptr(), RENAME_NOREPLACE) };
    if rc == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    if dst.exists() {
        return Err(format!(
            "commit refused: output already exists: {}",
            dst.display()
        ));
    }
    // ENOSYS=38 / EINVAL=22 — fall back to plain rename after exists check.
    if matches!(err.raw_os_error(), Some(22) | Some(38)) {
        fs::rename(src, dst).map_err(|e| format!("commit rename failed: {e}"))?;
        return Ok(());
    }
    Err(format!("commit rename failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp_base() -> PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("geoforge-path-{n}"));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn rejects_output_inside_input() {
        let base = tmp_base();
        let input = base.join("in");
        fs::create_dir_all(&input).unwrap();
        fs::write(input.join("sentinel.txt"), b"keep").unwrap();
        let out = input.join("out");
        let err = validate_io_paths(&input, &out, "task-abc").unwrap_err();
        assert!(err.contains("inside"), "{err}");
        assert_eq!(fs::read(input.join("sentinel.txt")).unwrap(), b"keep");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn rejects_same_path() {
        let base = tmp_base();
        let input = base.join("same");
        fs::create_dir_all(&input).unwrap();
        let err = validate_io_paths(&input, &input, "task-abc").unwrap_err();
        assert!(err.contains("equal"), "{err}");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn rejects_existing_output() {
        let base = tmp_base();
        let input = base.join("in");
        let output = base.join("out");
        fs::create_dir_all(&input).unwrap();
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("other.bin"), b"x").unwrap();
        let err = validate_io_paths(&input, &output, "task-abc").unwrap_err();
        assert!(err.contains("already exists"), "{err}");
        assert_eq!(fs::read(output.join("other.bin")).unwrap(), b"x");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn accepts_sibling_output() {
        let base = tmp_base();
        let input = base.join("data in");
        let output = base.join("data out");
        fs::create_dir_all(&input).unwrap();
        let v = validate_io_paths(&input, &output, "task-ok1").unwrap();
        assert!(!v.output.exists());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn rename_no_replace_keeps_existing() {
        let base = tmp_base();
        let src = base.join("src");
        let dst = base.join("dst");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.txt"), b"a").unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(dst.join("b.txt"), b"b").unwrap();
        let err = rename_no_replace(&src, &dst).unwrap_err();
        assert!(err.contains("refused") || err.contains("failed"), "{err}");
        assert_eq!(fs::read(dst.join("b.txt")).unwrap(), b"b");
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn invalid_task_id() {
        assert!(validate_task_id("../x").is_err());
        assert!(validate_task_id("a/b").is_err());
        assert!(validate_task_id("task-ok").is_ok());
    }
}
