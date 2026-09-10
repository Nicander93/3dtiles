//! Debug dump of RebuildTree for acceptance prints.

use crate::types::RebuildTree;

pub fn format_level_counts(tree: &RebuildTree) -> String {
    let mut lines = Vec::new();
    for (level, count) in tree.level_counts() {
        lines.push(format!("L{level} {count}"));
    }
    lines.join("\n")
}

pub fn format_tree_detail(tree: &RebuildTree) -> String {
    let mut out = String::new();
    for (li, nodes) in tree.levels.iter().enumerate() {
        out.push_str(&format!("\n=== L{li} ({}) ===\n", nodes.len()));
        for n in nodes {
            let bounds = n
                .bounds
                .box_values
                .map(|b| {
                    format!(
                        "box=[{:.3},{:.3},{:.3}, hx={:.3},hy={:.3},hz={:.3}]",
                        b[0],
                        b[1],
                        b[2],
                        b[3].abs(),
                        b[7].abs(),
                        b[11].abs()
                    )
                })
                .unwrap_or_else(|| "box=<none>".into());
            let t = &n.world_transform.0;
            out.push_str(&format!(
                "  {id} grid=({gx},{gy})\n    {bounds}\n    transform=[{t0:.3},{t1:.3},{t2:.3},{t3:.3}, {t4:.3},{t5:.3},{t6:.3},{t7:.3}, {t8:.3},{t9:.3},{t10:.3},{t11:.3}, {t12:.3},{t13:.3},{t14:.3},{t15:.3}]\n    source_representation_ids={reps:?}\n    child_ids={children:?}\n",
                id = n.id,
                gx = n.grid_x,
                gy = n.grid_y,
                bounds = bounds,
                t0 = t[0], t1 = t[1], t2 = t[2], t3 = t[3],
                t4 = t[4], t5 = t[5], t6 = t[6], t7 = t[7],
                t8 = t[8], t9 = t[9], t10 = t[10], t11 = t[11],
                t12 = t[12], t13 = t[13], t14 = t[14], t15 = t[15],
                reps = n.source_representation_ids,
                children = n.child_ids,
            ));
        }
    }
    out
}

pub fn print_acceptance(tree: &RebuildTree) {
    println!("{}", format_level_counts(tree));
    print!("{}", format_tree_detail(tree));
}
