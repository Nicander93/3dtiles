type Point = [f64; 2];
fn cross(a: Point, b: Point, c: Point) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn on_segment(a: Point, b: Point, p: Point) -> bool {
    cross(a, b, p).abs() <= 1e-6
        && (0..2).all(|i| p[i] >= a[i].min(b[i]) - 1e-6 && p[i] <= a[i].max(b[i]) + 1e-6)
}
fn intersects(a: Point, b: Point, c: Point, d: Point) -> bool {
    let x = cross(a, b, c);
    let y = cross(a, b, d);
    let z = cross(c, d, a);
    let w = cross(c, d, b);
    (x * y < 0. && z * w < 0.)
        || on_segment(a, b, c)
        || on_segment(a, b, d)
        || on_segment(c, d, a)
        || on_segment(c, d, b)
}

/// Validate a CCW simple ring and partition only concave rings. No dependencies or holes.
pub fn decompose(ring: &[Point]) -> Result<Vec<Vec<Point>>, String> {
    let n = ring.len();
    for i in 0..n {
        for j in i + 1..n {
            if (ring[i][0] - ring[j][0]).hypot(ring[i][1] - ring[j][1]) < 1e-6 {
                return Err("clip polygon has repeated vertices".into());
            }
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if intersects(ring[i], ring[(i + 1) % n], ring[j], ring[(j + 1) % n]) {
                return Err("clip polygon must not self-intersect or touch itself".into());
            }
        }
    }
    if (0..n).all(|i| cross(ring[i], ring[(i + 1) % n], ring[(i + 2) % n]) >= 0.) {
        return Ok(vec![ring.to_vec()]);
    }
    let mut remaining: Vec<usize> = (0..n).collect();
    let mut parts = Vec::new();
    while remaining.len() > 3 {
        let len = remaining.len();
        let ear = (0..len)
            .find(|&i| {
                let a = remaining[(i + len - 1) % len];
                let b = remaining[i];
                let c = remaining[(i + 1) % len];
                cross(ring[a], ring[b], ring[c]) > 1e-10
                    && !remaining.iter().any(|&p| {
                        p != a
                            && p != b
                            && p != c
                            && cross(ring[a], ring[b], ring[p]) >= -1e-10
                            && cross(ring[b], ring[c], ring[p]) >= -1e-10
                            && cross(ring[c], ring[a], ring[p]) >= -1e-10
                    })
            })
            .ok_or(
                "clip polygon cannot be decomposed reliably; adjust nearly collinear vertices",
            )?;
        parts.push(vec![
            ring[remaining[(ear + len - 1) % len]],
            ring[remaining[ear]],
            ring[remaining[(ear + 1) % len]],
        ]);
        remaining.remove(ear);
    }
    parts.push(remaining.iter().map(|&i| ring[i]).collect());
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_crossings_touches_repeated_vertices_and_overlapping_edges() {
        for ring in [
            vec![[0., 0.], [3., 0.], [0., 2.], [2., 2.]],
            vec![[0., 0.], [3., 0.], [3., 3.], [1., 0.], [0., 3.]],
            vec![[0., 0.], [3., 0.], [3., 3.], [3., 0.], [0., 3.]],
            vec![[0., 0.], [3., 0.], [1., 0.], [1., 3.], [0., 3.]],
        ] {
            assert!(decompose(&ring).is_err(), "{ring:?}");
        }
    }
    #[test]
    fn u_shape_and_collinear_boundary_preserve_area() {
        let ring = vec![
            [0., 0.],
            [2., 0.],
            [4., 0.],
            [4., 4.],
            [3., 4.],
            [3., 1.],
            [1., 1.],
            [1., 4.],
            [0., 4.],
        ];
        let parts = decompose(&ring).unwrap();
        let area: f64 = parts
            .iter()
            .map(|p| {
                (0..p.len())
                    .map(|i| {
                        let a = p[i];
                        let b = p[(i + 1) % p.len()];
                        (a[0] * b[1] - b[0] * a[1]) * 0.5
                    })
                    .sum::<f64>()
            })
            .sum();
        assert!((area - 10.).abs() < 1e-10);
    }
}
