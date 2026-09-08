//! Post-process top-level reconstruction (`rebuild-top`) CLI surface.
//!
//! v0 intentionally stubs the geometry merge; the interface is fixed so the
//! desktop UI and docs can depend on it.

use clap::{Arg, ArgAction, ArgMatches, Command};
use std::path::Path;
use std::process::ExitCode;

/// Build the `rebuild-top` subcommand.
pub fn command() -> Command {
    Command::new("rebuild-top")
        .about(
            "Post-process top-level reconstruction for an existing 3D Tiles tileset \
             (after OSGB→3D Tiles conversion). Not implemented yet — scaffold only.",
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

/// Run the stub. Always exits with a controlled non-zero code until implemented.
pub fn run(matches: &ArgMatches) -> ExitCode {
    let input = matches
        .get_one::<String>("input")
        .map(String::as_str)
        .unwrap_or("");
    let output = matches
        .get_one::<String>("output")
        .map(String::as_str)
        .unwrap_or("");
    let levels = matches
        .get_one::<String>("levels")
        .map(String::as_str)
        .unwrap_or("1");
    let simplify = matches
        .get_one::<String>("simplify")
        .map(String::as_str)
        .unwrap_or("0.5");
    let texture_scale = matches
        .get_one::<String>("texture-scale")
        .map(String::as_str)
        .unwrap_or("0.5");

    eprintln!("rebuild-top: not implemented yet (v0 scaffold stub)");
    eprintln!("  planned interface:");
    eprintln!(
        "    _3dtile rebuild-top -i <tileset_dir> -o <out_dir> --levels N [--simplify 0.5] [--texture-scale 0.5]"
    );
    eprintln!("  received:");
    eprintln!("    input         = {input}");
    eprintln!("    output        = {output}");
    eprintln!("    levels        = {levels}");
    eprintln!("    simplify      = {simplify}");
    eprintln!("    texture-scale = {texture_scale}");
    if matches.get_flag("verbose") {
        eprintln!("    verbose       = true");
    }
    if !input.is_empty() {
        let tileset = Path::new(input).join("tileset.json");
        if tileset.is_file() {
            eprintln!("  note: found tileset.json under input (good for a future run)");
        } else {
            eprintln!("  note: tileset.json not found under input (ok for stub)");
        }
    }
    eprintln!("  see docs/REBUILD_TOP.md for the algorithm design.");
    ExitCode::from(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Command;

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
        assert!(s.contains("Post-process") || s.contains("post-process") || s.contains("tileset"));
    }

    #[test]
    fn stub_run_returns_nonzero() {
        let m = command()
            .try_get_matches_from([
                "rebuild-top",
                "-i",
                "/no/such/in",
                "-o",
                "/no/such/out",
            ])
            .unwrap();
        assert_eq!(run(&m), ExitCode::from(2));
    }
}
