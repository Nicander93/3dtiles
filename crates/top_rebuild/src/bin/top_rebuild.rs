//! Release-oriented TopRebuild CLI for Processor (Phase 9).
//!
//! Stable args match processor needs: input tileset dir, output dir, levels,
//! optional KTX2 / texture & triangle budgets. Default path for formal V1
//! (no Python). Debug extras live in `top_rebuild_debug`.

use clap::Parser;
use std::path::PathBuf;
use top_rebuild::{
    find_basisu, probe_tileset_structure, rebuild_tileset, TreeBuildOptions, WriteOptions,
};

#[derive(Parser, Debug)]
#[command(
    name = "top_rebuild",
    about = "GeoForge TopRebuild core — Proxy HLOD pyramid (release CLI)"
)]
struct Args {
    /// Input tileset directory (or tileset.json path)
    #[arg(short = 'i', long = "input")]
    input: PathBuf,

    /// Output directory (created / replaced)
    #[arg(short = 'o', long = "output", visible_alias = "out")]
    output: PathBuf,

    /// Pyramid merge levels beyond L0 (1 = L1 proxies, 2 = L1+L2). Default: until root.
    #[arg(long)]
    levels: Option<u32>,

    #[arg(long, default_value_t = 0.5)]
    source_error_ratio: f64,

    #[arg(long, default_value_t = 4000)]
    l1_max_triangles: u64,

    #[arg(long, default_value_t = 2000)]
    l2_max_triangles: u64,

    #[arg(long, default_value_t = 2.0)]
    target_error: f64,

    #[arg(long, default_value_t = 1024)]
    max_texture_size: u32,

    #[arg(long, default_value_t = 8 * 1024 * 1024)]
    max_texture_bytes: u64,

    #[arg(long, default_value_t = 16 * 1024 * 1024)]
    max_glb_bytes: u64,

    /// Enable optional KTX2 via basisu (not required for correctness)
    #[arg(long, default_value_t = false)]
    ktx2: bool,

    /// Write proxy content as GLB instead of B3DM
    #[arg(long, default_value_t = false)]
    glb: bool,

    /// Fail hard on budget overruns (default: warn)
    #[arg(long, default_value_t = false)]
    strict_budget: bool,

    #[arg(long, default_value_t = 1.0)]
    gap_warn_meters: f64,

    /// Box segments when synthesizing empty leaf content (fixtures only)
    #[arg(long, default_value_t = 6)]
    segments: u32,

    /// Inject solid test textures when synthesizing empty leaves (off for release)
    #[arg(long, default_value_t = false)]
    inject_test_textures: bool,
}

fn main() {
    let args = Args::parse();
    if let Err(e) = run(args) {
        eprintln!("top_rebuild error: {e}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> top_rebuild::Result<()> {
    println!(
        "top_rebuild engine=rust-core input={} output={}",
        args.input.display(),
        args.output.display()
    );

    let tree_opts = TreeBuildOptions {
        source_error_ratio: args.source_error_ratio,
        target_proxy_error: None,
        max_levels: args.levels,
    };
    let write_opts = WriteOptions {
        pack_as_b3dm: !args.glb,
        l1_max_triangles: args.l1_max_triangles,
        l2_max_triangles: args.l2_max_triangles,
        target_error_meters: args.target_error,
        synthesize_if_empty: true,
        box_segments: args.segments,
        source_error_ratio: args.source_error_ratio,
        max_texture_size: args.max_texture_size,
        max_texture_bytes: args.max_texture_bytes,
        max_glb_bytes: args.max_glb_bytes,
        enable_ktx2: args.ktx2,
        lock_border: true,
        strict_budget: args.strict_budget,
        inject_test_textures: args.inject_test_textures,
        gap_warn_meters: args.gap_warn_meters,
    };

    if args.ktx2 {
        match find_basisu() {
            Some(p) => println!("basisu {}", p.display()),
            None => println!("basisu UNAVAILABLE (KTX2 will fall back to PNG with warning)"),
        }
    }

    let report = rebuild_tileset(&args.input, &args.output, &tree_opts, &write_opts)?;
    println!("tileset {}", report.tileset_path.display());
    println!("metrics {}", report.metrics_path.display());
    for (lvl, count) in &report.level_counts {
        println!("L{lvl} {count}");
    }
    println!(
        "gaps maxGap={:.6} P95Gap={:.6} pairs={}",
        report.gap.max_gap, report.gap.p95_gap, report.gap.pair_count
    );
    println!(
        "totals tris_after={} texture_bytes={} glb_bytes={}",
        report.total_triangles_after, report.total_texture_bytes, report.total_glb_bytes
    );
    for w in &report.warnings {
        println!("warning {w}");
    }
    let probe = probe_tileset_structure(&report.tileset_path)?;
    println!(
        "probe proxies={} leaf_external={} replace={} max_depth={} ge_count={}",
        probe.proxy_nodes,
        probe.leaf_external,
        probe.replace_count,
        probe.max_depth,
        probe.geometric_errors.len()
    );
    Ok(())
}
