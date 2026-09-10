//! processor CLI — `run --task`, `convert-osgb`, `process-tileset`, `scan-osgb`.

use clap::{Parser, Subcommand};
use processor::{
    run_task, scan_osgb, CancelFlag, TaskConfig, EXIT_FAILED,
};
use serde_json::json;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "processor", about = "GeoForge V1 task processor")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run a task from JSON TaskConfig file (plan §7.1).
    Run {
        #[arg(long)]
        task: PathBuf,
    },
    /// Convenience: convert-osgb without a task file.
    ConvertOsgb {
        #[arg(short = 'i', long)]
        input: PathBuf,
        #[arg(short = 'o', long)]
        output: PathBuf,
        #[arg(long, default_value = "keep")]
        texture: String,
        #[arg(long, default_value_t = false)]
        rebuild_top: bool,
        #[arg(long)]
        task_id: Option<String>,
    },
    /// Convenience: process-tileset without a task file.
    ProcessTileset {
        #[arg(short = 'i', long)]
        input: PathBuf,
        #[arg(short = 'o', long)]
        output: PathBuf,
        #[arg(long, default_value = "keep")]
        texture: String,
        #[arg(long, default_value_t = false)]
        rebuild_top: bool,
        /// Merge levels beyond L0 (1=L1 only). Use 0 for full pyramid until single root (Rust engine).
        #[arg(long, default_value_t = 1)]
        levels: i64,
        #[arg(long)]
        task_id: Option<String>,
    },
    /// OSGB scan (no Python). Prints JSON to stdout (not JSONL events).
    ScanOsgb {
        #[arg(long)]
        path: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { task } => {
            let text = match std::fs::read_to_string(&task) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("failed to read task file: {e}");
                    return ExitCode::from(EXIT_FAILED as u8);
                }
            };
            let config: TaskConfig = match serde_json::from_str(&text) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("invalid TaskConfig: {e}");
                    return ExitCode::from(EXIT_FAILED as u8);
                }
            };
            let code = run_task(config, CancelFlag::new()).exit_code;
            ExitCode::from(code as u8)
        }
        Commands::ConvertOsgb {
            input,
            output,
            texture,
            rebuild_top,
            task_id,
        } => {
            let id = task_id.unwrap_or_else(|| format!("cli-{}", chrono_stamp()));
            let config = TaskConfig {
                schema_version: Some(1),
                task_id: id,
                operation: "convert-osgb".into(),
                input: processor::protocol::PathRef {
                    path: input.to_string_lossy().into_owned(),
                },
                output: processor::protocol::PathRef {
                    path: output.to_string_lossy().into_owned(),
                },
                options: json!({
                    "texture": { "mode": texture },
                    "rebuildTop": { "enabled": rebuild_top },
                }),
            };
            let code = run_task(config, CancelFlag::new()).exit_code;
            ExitCode::from(code as u8)
        }
        Commands::ProcessTileset {
            input,
            output,
            texture,
            rebuild_top,
            levels,
            task_id,
        } => {
            let id = task_id.unwrap_or_else(|| format!("cli-{}", chrono_stamp()));
            let config = TaskConfig {
                schema_version: Some(1),
                task_id: id,
                operation: "process-tileset".into(),
                input: processor::protocol::PathRef {
                    path: input.to_string_lossy().into_owned(),
                },
                output: processor::protocol::PathRef {
                    path: output.to_string_lossy().into_owned(),
                },
                options: json!({
                    "texture": { "mode": texture },
                    "rebuildTop": { "enabled": rebuild_top, "levels": levels },
                }),
            };
            let code = run_task(config, CancelFlag::new()).exit_code;
            ExitCode::from(code as u8)
        }
        Commands::ScanOsgb { path } => {
            let result = scan_osgb(&path.to_string_lossy());
            println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
            if result.get("valid").and_then(|v| v.as_bool()).unwrap_or(false) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_FAILED as u8)
            }
        }
    }
}

fn chrono_stamp() -> String {
    chrono::Utc::now().format("%Y%m%d%H%M%S").to_string()
}
