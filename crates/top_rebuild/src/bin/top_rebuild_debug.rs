//! CLI: tree (P5) + proxy (P6) + HLOD rebuild (P7) + texture/budget metrics (P8).

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use top_rebuild::b3dm::pack_glb_as_b3dm;
use top_rebuild::glb::{make_box_primitive, write_glb};
use top_rebuild::{
    build_from_tileset, build_proxy_to_file, find_basisu, print_acceptance, probe_tileset_structure,
    rebuild_tileset, ChildContent, Mat4d, ProxyBudget, TreeBuildOptions, WriteOptions,
};

#[derive(Parser, Debug)]
#[command(name = "top_rebuild_debug", about = "TopRebuild debug CLI (tree + proxy + rebuild)")]
struct Args {
    #[command(subcommand)]
    cmd: Option<Commands>,

    #[arg(short, long)]
    input: Option<PathBuf>,

    #[arg(long, default_value_t = 0.5)]
    source_error_ratio: f64,

    #[arg(long)]
    max_levels: Option<u32>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Tree {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(long, default_value_t = 0.5)]
        source_error_ratio: f64,
        #[arg(long)]
        max_levels: Option<u32>,
    },
    Proxy {
        #[arg(long)]
        fixture: Option<PathBuf>,
        #[arg(short, long)]
        out: PathBuf,
        #[arg(long, default_value_t = 2000)]
        max_triangles: u64,
        #[arg(long, default_value_t = 2.0)]
        target_error: f64,
        #[arg(long, default_value_t = 8)]
        segments: u32,
        #[arg(long, default_value_t = 1024)]
        max_texture_size: u32,
        #[arg(long, default_value_t = 8 * 1024 * 1024)]
        max_texture_bytes: u64,
        #[arg(long, default_value_t = 16 * 1024 * 1024)]
        max_glb_bytes: u64,
        #[arg(long, default_value_t = false)]
        ktx2: bool,
    },
    /// Phase 7–8: full HLOD rebuild → tileset.json + rebuild_metrics.json
    Rebuild {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
        #[arg(long, default_value_t = 4000)]
        l1_max_triangles: u64,
        #[arg(long, default_value_t = 2000)]
        l2_max_triangles: u64,
        #[arg(long, default_value_t = 2.0)]
        target_error: f64,
        #[arg(long, default_value_t = 6)]
        segments: u32,
        #[arg(long, default_value_t = false)]
        glb: bool,
        #[arg(long, default_value_t = 0.5)]
        source_error_ratio: f64,
        #[arg(long, default_value_t = 1024)]
        max_texture_size: u32,
        #[arg(long, default_value_t = 8 * 1024 * 1024)]
        max_texture_bytes: u64,
        #[arg(long, default_value_t = 16 * 1024 * 1024)]
        max_glb_bytes: u64,
        #[arg(long, default_value_t = false)]
        ktx2: bool,
        #[arg(long, default_value_t = false)]
        strict_budget: bool,
        #[arg(long, default_value_t = 1.0)]
        gap_warn_meters: f64,
        #[arg(long, default_value_t = true)]
        inject_test_textures: bool,
    },
}

fn main() {
    let args = Args::parse();
    match args.cmd {
        Some(Commands::Tree {
            input,
            source_error_ratio,
            max_levels,
        }) => run_tree(&input, source_error_ratio, max_levels),
        Some(Commands::Proxy {
            fixture,
            out,
            max_triangles,
            target_error,
            segments,
            max_texture_size,
            max_texture_bytes,
            max_glb_bytes,
            ktx2,
        }) => {
            if let Err(e) = run_proxy(
                fixture.as_ref(),
                &out,
                max_triangles,
                target_error,
                segments,
                max_texture_size,
                max_texture_bytes,
                max_glb_bytes,
                ktx2,
            ) {
                eprintln!("proxy error: {e}");
                std::process::exit(1);
            }
        }
        Some(Commands::Rebuild {
            input,
            out,
            l1_max_triangles,
            l2_max_triangles,
            target_error,
            segments,
            glb,
            source_error_ratio,
            max_texture_size,
            max_texture_bytes,
            max_glb_bytes,
            ktx2,
            strict_budget,
            gap_warn_meters,
            inject_test_textures,
        }) => {
            if let Err(e) = run_rebuild(
                &input,
                &out,
                l1_max_triangles,
                l2_max_triangles,
                target_error,
                segments,
                glb,
                source_error_ratio,
                max_texture_size,
                max_texture_bytes,
                max_glb_bytes,
                ktx2,
                strict_budget,
                gap_warn_meters,
                inject_test_textures,
            ) {
                eprintln!("rebuild error: {e}");
                std::process::exit(1);
            }
        }
        None => {
            let input = args.input.unwrap_or_else(|| {
                eprintln!("provide --input or a subcommand (tree|proxy|rebuild)");
                std::process::exit(2);
            });
            run_tree(&input, args.source_error_ratio, args.max_levels);
        }
    }
}

fn run_tree(input: &std::path::Path, source_error_ratio: f64, max_levels: Option<u32>) {
    let opts = TreeBuildOptions {
        source_error_ratio,
        target_proxy_error: None,
        max_levels,
    };
    match build_from_tileset(input, &opts) {
        Ok(tree) => print_acceptance(&tree),
        Err(e) => {
            eprintln!("top_rebuild_debug error: {e}");
            std::process::exit(1);
        }
    }
}

fn run_rebuild(
    input: &std::path::Path,
    out: &std::path::Path,
    l1_max_triangles: u64,
    l2_max_triangles: u64,
    target_error: f64,
    segments: u32,
    glb: bool,
    source_error_ratio: f64,
    max_texture_size: u32,
    max_texture_bytes: u64,
    max_glb_bytes: u64,
    ktx2: bool,
    strict_budget: bool,
    gap_warn_meters: f64,
    inject_test_textures: bool,
) -> top_rebuild::Result<()> {
    let tree_opts = TreeBuildOptions {
        source_error_ratio,
        target_proxy_error: None,
        max_levels: None,
    };
    let write_opts = WriteOptions {
        pack_as_b3dm: !glb,
        l1_max_triangles,
        l2_max_triangles,
        target_error_meters: target_error,
        synthesize_if_empty: true,
        box_segments: segments,
        source_error_ratio,
        max_texture_size,
        max_texture_bytes,
        max_glb_bytes,
        enable_ktx2: ktx2,
        lock_border: true,
        strict_budget,
        inject_test_textures,
        gap_warn_meters,
    };
    if ktx2 {
        match find_basisu() {
            Some(p) => println!("basisu {}", p.display()),
            None => println!("basisu UNAVAILABLE (KTX2 will fall back to PNG with warning)"),
        }
    }
    let report = rebuild_tileset(input, out, &tree_opts, &write_opts)?;
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
        "textures unique={} dedup_removed={} bytes={} ktx2_avail={} ktx2_enc={}",
        report.texture.unique_count,
        report.texture.dedup_removed,
        report.total_texture_bytes,
        report.texture.ktx2_available,
        report.texture.ktx2_encoded
    );
    println!(
        "totals tris_after={} texture_bytes={} glb_bytes={}",
        report.total_triangles_after, report.total_texture_bytes, report.total_glb_bytes
    );
    for p in &report.proxies {
        println!(
            "proxy {} level={} tris {} -> {} (target {}) simp_err={:.6} ge={:.3} tex_b={} glb_b={} maxGap={:.4} uri={}",
            p.node_id,
            p.level,
            p.triangles_before,
            p.triangles_after,
            p.triangles_target,
            p.simplification_error_meters,
            p.geometric_error,
            p.texture_bytes,
            p.glb_bytes,
            p.max_gap,
            p.content_uri
        );
    }
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

fn run_proxy(
    fixture: Option<&PathBuf>,
    out: &std::path::Path,
    max_triangles: u64,
    target_error: f64,
    segments: u32,
    max_texture_size: u32,
    max_texture_bytes: u64,
    max_glb_bytes: u64,
    ktx2: bool,
) -> top_rebuild::Result<()> {
    let work = out
        .parent()
        .map(|p| p.join("_proxy_children"))
        .unwrap_or_else(|| PathBuf::from("_proxy_children"));
    std::fs::create_dir_all(&work)?;

    let (paths, xforms) = if let Some(fix) = fixture {
        let mut paths = Vec::new();
        for i in 0..4 {
            let b = fix.join(format!("child_{i}.b3dm"));
            let g = fix.join(format!("child_{i}.glb"));
            if b.exists() {
                paths.push(b);
            } else if g.exists() {
                paths.push(g);
            } else {
                return Err(top_rebuild::TopRebuildError::Other(format!(
                    "fixture missing child_{i}.b3dm/.glb under {}",
                    fix.display()
                )));
            }
        }
        let centers = [
            (50.0, 50.0, 10.0),
            (150.0, 50.0, 10.0),
            (50.0, 150.0, 10.0),
            (150.0, 150.0, 10.0),
        ];
        let xforms: Vec<_> = centers
            .iter()
            .map(|(x, y, z)| Mat4d::translation(*x, *y, *z))
            .collect();
        (paths, xforms)
    } else {
        let prim = make_box_primitive(20.0, 20.0, 5.0, segments, "mat0:0.800,0.800,0.800,1.000");
        let glb = write_glb(&[prim])?;
        let b3dm = pack_glb_as_b3dm(&glb)?;
        let centers = [
            (50.0, 50.0, 10.0),
            (150.0, 50.0, 10.0),
            (50.0, 150.0, 10.0),
            (150.0, 150.0, 10.0),
        ];
        let mut paths = Vec::new();
        let mut xforms = Vec::new();
        for (i, (cx, cy, cz)) in centers.iter().enumerate() {
            let p = work.join(format!("child_{i}.b3dm"));
            std::fs::write(&p, &b3dm)?;
            paths.push(p);
            xforms.push(Mat4d::translation(*cx, *cy, *cz));
        }
        (paths, xforms)
    };

    let parent = Mat4d::translation(100.0, 100.0, 10.0);
    let children: Vec<_> = paths
        .iter()
        .zip(xforms.iter())
        .map(|(p, t)| ChildContent {
            content_path: p.clone(),
            world_transform: t.clone(),
        })
        .collect();
    let budget = ProxyBudget {
        max_triangles,
        target_error_meters: target_error,
        max_texture_size,
        max_texture_bytes,
        max_glb_bytes,
        enable_ktx2: ktx2,
        ..Default::default()
    };
    let result = build_proxy_to_file(&children, &parent, &budget, out)?;
    println!("proxy_glb {}", out.display());
    println!("triangles_before {}", result.triangles_before);
    println!("triangles_after {}", result.triangles_after);
    println!("triangles_target {}", result.triangles_target);
    println!(
        "simplification_error_meters {:.6}",
        result.simplification_error_meters
    );
    println!("group_count {}", result.group_count);
    println!(
        "gaps maxGap={:.6} P95Gap={:.6}",
        result.gap.max_gap, result.gap.p95_gap
    );
    println!(
        "texture_bytes {} glb_bytes {} lock_border={}",
        result.texture_bytes, result.glb_bytes_len, result.lock_border
    );
    for w in &result.warnings {
        println!("warning {w}");
    }
    Ok(())
}
