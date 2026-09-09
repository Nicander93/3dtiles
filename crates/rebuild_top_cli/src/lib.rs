//! Post-process top-level reconstruction (`rebuild-top`) CLI.
//!
//! Dispatches to `tools/rebuild_top/rebuild_top.py` for the v0 merge
//! implementation (b3dm/GLB merge + tileset rewrite).

use clap::{Arg, ArgAction, ArgMatches, Command};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcCommand, ExitCode};

/// Build the `rebuild-top` subcommand.
pub fn command() -> Command {
    Command::new("rebuild-top")
        .about(
            "Post-process top-level reconstruction for an existing 3D Tiles tileset \
             (after OSGB→3D Tiles conversion).",
        )
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .value_name("TILESET_DIR")
                .help("Input tileset directory containing tileset.json")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("OUT_DIR")
                .help("Output directory (new tileset; does not overwrite input)")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("levels")
                .long("levels")
                .value_name("N")
                .help("Number of pyramid merge levels (1 ≈ ~1/4 root tiles via 2×2)")
                .default_value("1")
                .num_args(1),
        )
        .arg(
            Arg::new("simplify")
                .long("simplify")
                .value_name("RATIO")
                .help("Mesh simplify keep-ratio in (0, 1], e.g. 0.5")
                .default_value("0.5")
                .num_args(1),
        )
        .arg(
            Arg::new("texture-scale")
                .long("texture-scale")
                .value_name("SCALE")
                .help("Texture downsample scale in (0, 1], e.g. 0.5")
                .default_value("0.5")
                .num_args(1),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Verbose logging")
                .action(ArgAction::SetTrue),
        )
}

fn find_repo_root() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().take(8) {
            let cand = ancestor.join("tools/rebuild_top/rebuild_top.py");
            if cand.is_file() {
                return Some(ancestor.to_path_buf());
            }
        }
    }
    let cwd = std::env::current_dir().ok()?;
    for ancestor in cwd.ancestors().take(8) {
        let cand = ancestor.join("tools/rebuild_top/rebuild_top.py");
        if cand.is_file() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

fn find_python(repo: &Path) -> PathBuf {
    let venv = repo.join(".venv/bin/python");
    if venv.is_file() {
        return venv;
    }
    // workspace fallback used in CI/dev box
    let alt = PathBuf::from("/workspace/venv-3dtiles/bin/python");
    if alt.is_file() {
        return alt;
    }
    PathBuf::from("python3")
}

/// Run rebuild-top by invoking the Python implementation.
pub fn run(matches: &ArgMatches) -> ExitCode {
    let input = matches.get_one::<String>("input").map(String::as_str).unwrap_or("");
    let output = matches.get_one::<String>("output").map(String::as_str).unwrap_or("");
    let levels = matches.get_one::<String>("levels").map(String::as_str).unwrap_or("1");
    let simplify = matches.get_one::<String>("simplify").map(String::as_str).unwrap_or("0.5");
    let texture_scale = matches
        .get_one::<String>("texture-scale")
        .map(String::as_str)
        .unwrap_or("0.5");
    let verbose = matches.get_flag("verbose");

    let Some(repo) = find_repo_root() else {
        eprintln!("rebuild-top: cannot find tools/rebuild_top/rebuild_top.py (run from repo checkout)");
        return ExitCode::from(2);
    };
    let script = repo.join("tools/rebuild_top/rebuild_top.py");
    let python = find_python(&repo);

    let mut cmd = ProcCommand::new(&python);
    cmd.arg(&script)
        .arg("-i")
        .arg(input)
        .arg("-o")
        .arg(output)
        .arg("--levels")
        .arg(levels)
        .arg("--simplify")
        .arg(simplify)
        .arg("--texture-scale")
        .arg(texture_scale);
    if verbose {
        cmd.arg("-v");
    }

    match cmd.status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => ExitCode::from(status.code().unwrap_or(1) as u8),
        Err(err) => {
            eprintln!("rebuild-top: failed to launch {:?}: {}", python, err);
            eprintln!("  hint: create .venv and pip install numpy pillow trimesh");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> Command {
        Command::new("test").subcommand(command())
    }

    #[test]
    fn parses_rebuild_top_args() {
        let m = app()
            .try_get_matches_from([
                "test",
                "rebuild-top",
                "-i",
                "/tmp/in",
                "-o",
                "/tmp/out",
                "--levels",
                "2",
                "--simplify",
                "0.25",
                "--texture-scale",
                "0.5",
            ])
            .expect("parse");
        let sub = m.subcommand().expect("subcommand");
        assert_eq!(sub.0, "rebuild-top");
        let sm = sub.1;
        assert_eq!(sm.get_one::<String>("input").unwrap(), "/tmp/in");
        assert_eq!(sm.get_one::<String>("output").unwrap(), "/tmp/out");
        assert_eq!(sm.get_one::<String>("levels").unwrap(), "2");
        assert_eq!(sm.get_one::<String>("simplify").unwrap(), "0.25");
        assert_eq!(sm.get_one::<String>("texture-scale").unwrap(), "0.5");
    }

    #[test]
    fn help_mentions_post_process() {
        let mut cmd = command();
        let mut help = Vec::new();
        cmd.write_long_help(&mut help).unwrap();
        let s = String::from_utf8(help).unwrap();
        assert!(s.to_lowercase().contains("post-process") || s.contains("tileset"));
    }
}
