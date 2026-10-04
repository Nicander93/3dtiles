use super::content::Counts;
use super::math::*;
use std::collections::BTreeMap;

type Layout = BTreeMap<String, (usize, usize)>;
pub struct Shading {
    pub uv: String,
    pub transform: [f64; 4],
}
impl Default for Shading {
    fn default() -> Self {
        Self {
            uv: "TEXCOORD_0".into(),
            transform: [1., 0., 0., 1.],
        }
    }
}
fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn subtract(a: Point, b: Point) -> Point {
    std::array::from_fn(|i| a[i] - b[i])
}
fn unit(p: Point) -> Option<Point> {
    let length = dot(p, p).sqrt();
    (length > 1e-12).then(|| p.map(|v| v / length))
}

fn triangulate(polygon: &[Vertex]) -> Vec<[Vertex; 3]> {
    (1..polygon.len().saturating_sub(1))
        .map(|i| {
            [
                polygon[0].clone(),
                polygon[i].clone(),
                polygon[i + 1].clone(),
            ]
        })
        .collect()
}
fn flatten_vertex(mut v: Vertex, height: f64, inverse: Matrix, offset: usize) -> Vertex {
    v.clip[2] = height;
    v.values[offset..offset + 3].copy_from_slice(&point(inverse, v.clip));
    v
}

/// Recalculate face normals and UV tangent frames only for modified surfaces and new walls.
fn shade(
    mut tri: [Vertex; 3],
    offset: usize,
    layout: &Layout,
    shading: &Shading,
) -> Option<[Vertex; 3]> {
    let p: Vec<Point> = tri
        .iter()
        .map(|v| v.values[offset..offset + 3].try_into().unwrap())
        .collect();
    let ab = subtract(p[1], p[0]);
    let ac = subtract(p[2], p[0]);
    let n = unit(cross(ab, ac))?;
    let mut tangent = None;
    if let Some((uv, _)) = layout.get(&shading.uv) {
        let delta = |v: &Vertex| {
            let u = v.values[*uv] - tri[0].values[*uv];
            let v = v.values[*uv + 1] - tri[0].values[*uv + 1];
            let m = shading.transform;
            [m[0] * u + m[1] * v, m[2] * u + m[3] * v]
        };
        let d1 = delta(&tri[1]);
        let d2 = delta(&tri[2]);
        let determinant = d1[0] * d2[1] - d2[0] * d1[1];
        if determinant.abs() > 1e-12 {
            let t: Point = std::array::from_fn(|i| (ab[i] * d2[1] - ac[i] * d1[1]) / determinant);
            let b: Point = std::array::from_fn(|i| (ac[i] * d1[0] - ab[i] * d2[0]) / determinant);
            if let Some(t) = unit(t) {
                tangent = Some((t, if dot(cross(n, t), b) < 0. { -1. } else { 1. }));
            }
        }
    }
    let (t, sign) = tangent.unwrap_or((unit(ab).unwrap(), 1.));
    for vertex in &mut tri {
        if let Some((normal, _)) = layout.get("NORMAL") {
            vertex.values[*normal..*normal + 3].copy_from_slice(&n);
        }
        if let Some((tangent, _)) = layout.get("TANGENT") {
            vertex.values[*tangent..*tangent + 3].copy_from_slice(&t);
            vertex.values[*tangent + 3] = sign;
        }
    }
    Some(tri)
}

pub fn flatten_triangle(
    region: &Region,
    input: [Vertex; 3],
    height: f64,
    inverse: Matrix,
    offset: usize,
    layout: &Layout,
    shading: &Shading,
    counts: &mut Counts,
) -> Result<Vec<[Vertex; 3]>, String> {
    let (inside, outside) = partition_triangle(region, input);
    let mut output: Vec<_> = outside.iter().flat_map(|p| triangulate(p)).collect();
    for inside in inside {
        let flat: Vec<_> = inside
            .iter()
            .cloned()
            .map(|v| flatten_vertex(v, height, inverse, offset))
            .collect();
        let flattened_before = counts.flattened;
        for tri in triangulate(&flat) {
            if let Some(tri) = shade(tri, offset, layout, shading) {
                output.push(tri);
                counts.flattened += 1;
            }
        }
        // Only ROI edges need walls; triangle fan edges and original interior seams do not.
        if counts.flattened == flattened_before || inside.len() < 3 {
            continue;
        }
        for i in 0..inside.len() {
            let mut a = inside[i].clone();
            let mut b = inside[(i + 1) % inside.len()].clone();
            let edge = (0..region.polygon.len()).find(|edge| {
                region.distance(*edge, a.clip).abs() < 1e-5
                    && region.distance(*edge, b.clip).abs() < 1e-5
                    && [a.clip, b.clip].iter().all(|p| {
                        let start = region.polygon[*edge];
                        let end = region.polygon[(*edge + 1) % region.polygon.len()];
                        let d = [end[0] - start[0], end[1] - start[1]];
                        let t = (p[0] - start[0]) * d[0] + (p[1] - start[1]) * d[1];
                        t >= -1e-6 && t <= d[0] * d[0] + d[1] * d[1] + 1e-6
                    })
            });
            let Some(edge) = edge else {
                continue;
            };
            let ra = region.polygon[edge];
            let rb = region.polygon[(edge + 1) % region.polygon.len()];
            if (b.clip[0] - a.clip[0]) * (rb[0] - ra[0]) + (b.clip[1] - a.clip[1]) * (rb[1] - ra[1])
                < 0.
            {
                std::mem::swap(&mut a, &mut b);
            }
            let da = a.clip[2] - height;
            let db = b.clip[2] - height;
            let segments = if da * db < 0. {
                let t = da / (da - db);
                let mid = Vertex {
                    values: a
                        .values
                        .iter()
                        .zip(&b.values)
                        .map(|(a, b)| a + (b - a) * t)
                        .collect(),
                    clip: std::array::from_fn(|j| a.clip[j] + (b.clip[j] - a.clip[j]) * t),
                };
                vec![(a.clone(), mid.clone()), (mid, b.clone())]
            } else {
                vec![(a.clone(), b.clone())]
            };
            for (a, b) in segments {
                if (a.clip[2] - height).abs().max((b.clip[2] - height).abs()) < 1e-5 {
                    continue;
                }
                let fa = flatten_vertex(a.clone(), height, inverse, offset);
                let fb = flatten_vertex(b.clone(), height, inverse, offset);
                let mut walls = [[a.clone(), fa, fb.clone()], [a.clone(), fb, b.clone()]];
                for tri in &mut walls {
                    if a.clip[2] + b.clip[2] < 2. * height {
                        tri.swap(1, 2);
                    }
                    for v in tri.iter_mut() {
                        let u = (v.clip[0] - a.clip[0]).hypot(v.clip[1] - a.clip[1]);
                        for name in ["TEXCOORD_0", "TEXCOORD_1"] {
                            if let Some((uv, _)) = layout.get(name) {
                                v.values[*uv] = u;
                                v.values[*uv + 1] = v.clip[2] - height;
                            }
                        }
                    }
                    if let Some(tri) = shade(tri.clone(), offset, layout, shading) {
                        output.push(tri);
                        counts.walls += 1;
                    }
                }
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concave_flatten_has_no_walls_on_decomposition_seams() {
        let polygon = vec![[0., 0.], [3., 0.], [3., 1.], [1., 1.], [1., 3.], [0., 3.]];
        let region = Region {
            frame: IDENTITY,
            parts: super::super::polygon::decompose(&polygon).unwrap(),
            polygon,
        };
        let v = |x, y| Vertex {
            values: vec![x, y, 2.],
            clip: [x, y, 2.],
        };
        let mut counts = Counts::default();
        let mut wall_area = 0.;
        for input in [
            [v(-1., -1.), v(4., -1.), v(4., 4.)],
            [v(-1., -1.), v(4., 4.), v(-1., 4.)],
        ] {
            let output = flatten_triangle(
                &region,
                input,
                0.,
                IDENTITY,
                0,
                &Layout::new(),
                &Shading::default(),
                &mut counts,
            )
            .unwrap();
            for tri in output {
                if tri.iter().all(|v| v.clip[2] == tri[0].clip[2]) {
                    continue;
                }
                assert!((0..region.polygon.len()).any(|edge| tri
                    .iter()
                    .all(|v| region.distance(edge, v.clip).abs() < 1e-8)));
                let ab = subtract(tri[1].clip, tri[0].clip);
                let ac = subtract(tri[2].clip, tri[0].clip);
                let normal = cross(ab, ac);
                wall_area += dot(normal, normal).sqrt() * 0.5;
            }
        }
        assert!((wall_area - 24.).abs() < 1e-8);
        assert!(counts.flattened > 0 && counts.walls > 0);
    }
    #[test]
    fn adjacent_triangles_generate_walls_only_on_roi_boundary() {
        let r = Region {
            frame: IDENTITY,
            parts: Vec::new(),
            polygon: vec![[-1., -1.], [1., -1.], [1., 1.], [-1., 1.]],
        };
        let v = |x, y| Vertex {
            values: vec![x, y, 2.],
            clip: [x, y, 2.],
        };
        let mut counts = Counts::default();
        for tri in [
            [v(-2., -2.), v(2., -2.), v(2., 2.)],
            [v(-2., -2.), v(2., 2.), v(-2., 2.)],
        ] {
            let result = flatten_triangle(
                &r,
                tri,
                0.,
                IDENTITY,
                0,
                &Layout::new(),
                &Shading::default(),
                &mut counts,
            )
            .unwrap();
            for tri in result {
                let z: Vec<_> = tri.iter().map(|v| v.clip[2]).collect();
                if z.iter().any(|z| *z == 0.) && z.iter().any(|z| *z == 2.) {
                    assert!((0..4).any(|e| tri.iter().all(|v| r.distance(e, v.clip).abs() < 1e-9)));
                }
            }
        }
        assert_eq!(counts.flattened, 2);
        assert_eq!(counts.walls, 8);
    }
    #[test]
    fn modified_face_recalculates_normal_and_mirrored_uv_tangent() {
        let layout = Layout::from([
            ("POSITION".into(), (0, 3)),
            ("NORMAL".into(), (3, 3)),
            ("TANGENT".into(), (6, 4)),
            ("TEXCOORD_0".into(), (10, 2)),
        ]);
        let v = |x, y, u, v| Vertex {
            clip: [x, y, 0.],
            values: vec![x, y, 0., 0., 1., 0., 1., 0., 0., 1., u, v],
        };
        let tri = shade(
            [v(0., 0., 0., 0.), v(1., 0., 0., 1.), v(0., 1., 1., 0.)],
            0,
            &layout,
            &Shading::default(),
        )
        .unwrap();
        for v in tri {
            assert_eq!(&v.values[3..6], &[0., 0., 1.]);
            assert_eq!(&v.values[6..10], &[0., 1., 0., -1.]);
        }
    }
}
