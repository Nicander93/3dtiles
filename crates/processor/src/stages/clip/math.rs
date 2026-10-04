use serde_json::Value;

pub type Point = [f64; 3];
pub type Matrix = [f64; 16];
pub const IDENTITY: Matrix = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];
pub const Y_UP: Matrix = [
    1., 0., 0., 0., 0., 0., 1., 0., 0., -1., 0., 0., 0., 0., 0., 1.,
];

pub fn array<const N: usize>(v: &Value) -> Result<[f64; N], String> {
    let a = v
        .as_array()
        .filter(|a| a.len() == N)
        .ok_or("invalid numeric array")?;
    let mut out = [0.; N];
    for (i, n) in a.iter().enumerate() {
        out[i] = n
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or("non-finite number")?;
    }
    Ok(out)
}
pub fn multiply(a: Matrix, b: Matrix) -> Matrix {
    std::array::from_fn(|i| (0..4).map(|k| a[k * 4 + i % 4] * b[i / 4 * 4 + k]).sum())
}
pub fn point(m: Matrix, p: Point) -> Point {
    std::array::from_fn(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r])
}
pub fn transform(v: Option<&Value>) -> Result<Matrix, String> {
    let m = v.map(array::<16>).transpose()?.unwrap_or(IDENTITY);
    if m[3] != 0. || m[7] != 0. || m[11] != 0. || m[15] != 1. {
        return Err("clip requires affine transforms".into());
    }
    let det = m[0] * (m[5] * m[10] - m[9] * m[6]) - m[4] * (m[1] * m[10] - m[9] * m[2])
        + m[8] * (m[1] * m[6] - m[5] * m[2]);
    if det.abs() < 1e-15 {
        return Err("clip refuses singular transforms".into());
    }
    Ok(m)
}
pub fn node_transform(node: &Value) -> Result<Matrix, String> {
    if node.get("matrix").is_some() {
        if ["translation", "rotation", "scale"]
            .iter()
            .any(|k| node.get(k).is_some())
        {
            return Err("glTF node mixes matrix and TRS".into());
        }
        return transform(node.get("matrix"));
    }
    let t = node
        .get("translation")
        .map(array::<3>)
        .transpose()?
        .unwrap_or([0.; 3]);
    let s = node
        .get("scale")
        .map(array::<3>)
        .transpose()?
        .unwrap_or([1.; 3]);
    let [x, y, z, w] = node
        .get("rotation")
        .map(array::<4>)
        .transpose()?
        .unwrap_or([0., 0., 0., 1.]);
    if ((x * x + y * y + z * z + w * w) - 1.).abs() > 1e-5 {
        return Err("glTF quaternion must be normalized".into());
    }
    let m = [
        (1. - 2. * (y * y + z * z)) * s[0],
        2. * (x * y + z * w) * s[0],
        2. * (x * z - y * w) * s[0],
        0.,
        2. * (x * y - z * w) * s[1],
        (1. - 2. * (x * x + z * z)) * s[1],
        2. * (y * z + x * w) * s[1],
        0.,
        2. * (x * z + y * w) * s[2],
        2. * (y * z - x * w) * s[2],
        (1. - 2. * (x * x + y * y)) * s[2],
        0.,
        t[0],
        t[1],
        t[2],
        1.,
    ];
    transform(Some(&serde_json::json!(m)))
}
pub fn ecef(lon: f64, lat: f64) -> Point {
    let (s, c) = lat.to_radians().sin_cos();
    let (sl, cl) = lon.to_radians().sin_cos();
    let n = 6378137. / (1. - 6.6943799901413165e-3 * s * s).sqrt();
    [n * c * cl, n * c * sl, n * (1. - 6.6943799901413165e-3) * s]
}
#[derive(Clone)]
pub struct Region {
    pub frame: Matrix,
    pub polygon: Vec<[f64; 2]>,
    pub parts: Vec<Vec<[f64; 2]>>,
}
impl Region {
    pub fn parse(v: &Value) -> Result<Self, String> {
        if v.get("crs").is_some() {
            return Err(
                "GeoJSON CRS overrides are unsupported; provide WGS84 longitude/latitude".into(),
            );
        }
        let ring = match v["type"].as_str() {
            Some("rectangle") => {
                let [w, s, e, n] = array::<4>(&v["bounds"])?;
                if w >= e || s >= n {
                    return Err("rectangle bounds must be west,south,east,north".into());
                }
                vec![[w, s], [e, s], [e, n], [w, n]]
            }
            Some("Polygon") => {
                let rings = v["coordinates"]
                    .as_array()
                    .filter(|a| a.len() == 1)
                    .ok_or("clip supports one polygon ring without holes")?;
                let mut ring = rings[0]
                    .as_array()
                    .ok_or("invalid polygon ring")?
                    .iter()
                    .map(array::<2>)
                    .collect::<Result<Vec<_>, _>>()?;
                if ring.first() != ring.last() {
                    return Err("GeoJSON ring must be closed".into());
                }
                ring.pop();
                ring
            }
            Some("Feature") => return Self::parse(&v["geometry"]),
            _ => return Err("region must be rectangle or GeoJSON Polygon/Feature".into()),
        };
        if ring.len() < 3 || ring.len() > 256 {
            return Err("clip polygon requires 3..256 vertices".into());
        }
        if ring.iter().any(|p| p[0].abs() > 180. || p[1].abs() > 80.) {
            return Err("clip longitude must be within ±180°, latitude within ±80°".into());
        }
        let lon = ring.iter().map(|p| p[0]).sum::<f64>() / ring.len() as f64;
        let lat = ring.iter().map(|p| p[1]).sum::<f64>() / ring.len() as f64;
        let origin = ecef(lon, lat);
        let (sl, cl) = lon.to_radians().sin_cos();
        let (s, c) = lat.to_radians().sin_cos();
        let east = [-sl, cl, 0.];
        let north = [-s * cl, -s * sl, c];
        let up = [c * cl, c * sl, s];
        let frame = [
            east[0],
            north[0],
            up[0],
            0.,
            east[1],
            north[1],
            up[1],
            0.,
            east[2],
            north[2],
            up[2],
            0.,
            -dot(east, origin),
            -dot(north, origin),
            -dot(up, origin),
            1.,
        ];
        let mut polygon: Vec<_> = ring
            .iter()
            .map(|p| {
                let q = point(frame, ecef(p[0], p[1]));
                [q[0], q[1]]
            })
            .collect();
        if ring.iter().any(|p| (p[0] - lon).abs() > 1.)
            || polygon.iter().any(|p| p[0].hypot(p[1]) > 10_000.)
        {
            return Err(
                "clip region must fit within 10 km of its center and must not cross the date line"
                    .into(),
            );
        }
        let area: f64 = (0..polygon.len())
            .map(|i| {
                let a = polygon[i];
                let b = polygon[(i + 1) % polygon.len()];
                a[0] * b[1] - a[1] * b[0]
            })
            .sum();
        if area.abs() < 1e-4 {
            return Err("clip region has zero area".into());
        }
        if area < 0. {
            polygon.reverse();
        }
        let parts = super::polygon::decompose(&polygon)?;
        Ok(Self {
            frame,
            polygon,
            parts,
        })
    }
    pub fn pieces(&self) -> &[Vec<[f64; 2]>] {
        if self.parts.is_empty() {
            std::slice::from_ref(&self.polygon)
        } else {
            &self.parts
        }
    }
    pub fn contains(&self, p: Point, tolerance: f64) -> bool {
        self.pieces()
            .iter()
            .any(|part| (0..part.len()).all(|edge| edge_distance(part, edge, p) >= -tolerance))
    }
    pub fn excludes_bounds(&self, corners: &[Point]) -> bool {
        self.pieces().iter().all(|part| {
            (0..part.len()).any(|edge| corners.iter().all(|p| edge_distance(part, edge, *p) < 0.))
        })
    }
    pub fn distance(&self, edge: usize, p: Point) -> f64 {
        let a = self.polygon[edge];
        let b = self.polygon[(edge + 1) % self.polygon.len()];
        cross(a, b, [p[0], p[1]]) / (b[0] - a[0]).hypot(b[1] - a[1])
    }
}
fn edge_distance(polygon: &[[f64; 2]], edge: usize, p: Point) -> f64 {
    let a = polygon[edge];
    let b = polygon[(edge + 1) % polygon.len()];
    cross(a, b, [p[0], p[1]]) / (b[0] - a[0]).hypot(b[1] - a[1])
}
fn cross(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}
pub fn dot(a: Point, b: Point) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}
impl Bounds {
    pub fn new(p: Point) -> Self {
        Self { min: p, max: p }
    }
    pub fn union(self, b: Self) -> Self {
        Self {
            min: std::array::from_fn(|i| self.min[i].min(b.min[i])),
            max: std::array::from_fn(|i| self.max[i].max(b.max[i])),
        }
    }
    pub fn transformed(self, m: Matrix) -> Self {
        let mut b = Self::new(point(m, self.min));
        for mask in 0..8 {
            b = b.union(Self::new(point(
                m,
                std::array::from_fn(|i| {
                    if mask & (1 << i) == 0 {
                        self.min[i]
                    } else {
                        self.max[i]
                    }
                }),
            )));
        }
        b
    }
    pub fn json(self) -> Value {
        let c: Point = std::array::from_fn(|i| (self.min[i] + self.max[i]) * 0.5);
        let h: Point = std::array::from_fn(|i| ((self.max[i] - self.min[i]) * 0.5).max(1e-6));
        serde_json::json!({"box":[c[0],c[1],c[2],h[0],0.,0.,0.,h[1],0.,0.,0.,h[2]]})
    }
}
#[derive(Clone, Debug)]
pub struct Vertex {
    pub values: Vec<f64>,
    pub clip: Point,
}
impl Vertex {
    fn interpolate(&self, b: &Self, t: f64) -> Self {
        Self {
            values: self
                .values
                .iter()
                .zip(&b.values)
                .map(|(a, b)| a + (b - a) * t)
                .collect(),
            clip: std::array::from_fn(|i| self.clip[i] + (b.clip[i] - self.clip[i]) * t),
        }
    }
}
/// Inverse of an affine column-major transform, including non-uniform node scales.
pub fn inverse(m: Matrix) -> Result<Matrix, String> {
    let [a, b, c, d, e, f, g, h, i] = [m[0], m[4], m[8], m[1], m[5], m[9], m[2], m[6], m[10]];
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    if !det.is_finite() || det.abs() < 1e-15 {
        return Err("non-invertible flatten transform".into());
    }
    let mut out = IDENTITY;
    let rows = [
        [e * i - f * h, c * h - b * i, b * f - c * e],
        [f * g - d * i, a * i - c * g, c * d - a * f],
        [d * h - e * g, b * g - a * h, a * e - b * d],
    ];
    for r in 0..3 {
        for col in 0..3 {
            out[col * 4 + r] = rows[r][col] / det;
        }
    }
    for r in 0..3 {
        out[12 + r] = -(out[r] * m[12] + out[4 + r] * m[13] + out[8 + r] * m[14]);
    }
    Ok(out)
}

pub fn split_polygon(
    boundary: &[[f64; 2]],
    polygon: Vec<Vertex>,
    edge: usize,
) -> (Vec<Vertex>, Vec<Vertex>) {
    let mut inside = Vec::new();
    let mut outside = Vec::new();
    if polygon.is_empty() {
        return (inside, outside);
    }
    let mut previous = polygon.last().unwrap();
    let mut dp = edge_distance(boundary, edge, previous.clip);
    for current in &polygon {
        let dc = edge_distance(boundary, edge, current.clip);
        if (dp >= 0.) != (dc >= 0.) {
            let v = previous.interpolate(current, (dp / (dp - dc)).clamp(0., 1.));
            inside.push(v.clone());
            outside.push(v);
        }
        if dc >= 0. {
            inside.push(current.clone());
        } else {
            outside.push(current.clone());
        }
        previous = current;
        dp = dc;
    }
    (inside, outside)
}

/// Disjoint intersection and remainder of a source triangle against the complete region.
pub fn partition_triangle(
    region: &Region,
    vertices: [Vertex; 3],
) -> (Vec<Vec<Vertex>>, Vec<Vec<Vertex>>) {
    let mut outside = vec![vertices.to_vec()];
    let mut inside = Vec::new();
    for part in region.pieces() {
        let mut next = Vec::new();
        for fragment in outside {
            if (0..part.len()).any(|edge| {
                fragment
                    .iter()
                    .all(|v| edge_distance(part, edge, v.clip) < 0.)
            }) {
                next.push(fragment);
                continue;
            }
            let mut remaining = fragment;
            for edge in 0..part.len() {
                let (keep, discard) = split_polygon(part, remaining, edge);
                if discard.len() >= 3 {
                    next.push(discard);
                }
                remaining = keep;
                if remaining.len() < 3 {
                    break;
                }
            }
            if remaining.len() >= 3 {
                inside.push(remaining);
            }
        }
        outside = next;
    }
    (inside, outside)
}
pub fn clip_region(region: &Region, vertices: [Vertex; 3]) -> Vec<Vec<Vertex>> {
    region
        .pieces()
        .iter()
        .map(|part| {
            let mut polygon = vertices.to_vec();
            for edge in 0..part.len() {
                polygon = split_polygon(part, polygon, edge).0;
                if polygon.len() < 3 {
                    break;
                }
            }
            polygon
        })
        .filter(|p| p.len() >= 3)
        .collect()
}
#[cfg(test)]
pub fn clip_triangle(region: &Region, vertices: [Vertex; 3]) -> Vec<Vertex> {
    let mut polygon = vertices.to_vec();
    for edge in 0..region.polygon.len() {
        if polygon.is_empty() {
            break;
        }
        let mut result = Vec::new();
        let mut previous = polygon.last().unwrap();
        let mut dp = region.distance(edge, previous.clip);
        for current in &polygon {
            let dc = region.distance(edge, current.clip);
            if (dp >= 0.) != (dc >= 0.) {
                result.push(previous.interpolate(current, (dp / (dp - dc)).clamp(0., 1.)));
            }
            if dc >= 0. {
                result.push(current.clone());
            }
            previous = current;
            dp = dc;
        }
        polygon = result;
    }
    polygon
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concave_partition_preserves_area_and_notch() {
        let polygon = vec![[0., 0.], [3., 0.], [3., 1.], [1., 1.], [1., 3.], [0., 3.]];
        let region = Region {
            frame: IDENTITY,
            parts: super::super::polygon::decompose(&polygon).unwrap(),
            polygon,
        };
        let v = |x, y| Vertex {
            values: vec![x, y, x + y],
            clip: [x, y, 2.],
        };
        let area = |p: &[Vertex]| {
            (1..p.len().saturating_sub(1))
                .map(|i| {
                    cross(
                        [p[0].clip[0], p[0].clip[1]],
                        [p[i].clip[0], p[i].clip[1]],
                        [p[i + 1].clip[0], p[i + 1].clip[1]],
                    )
                    .abs()
                        * 0.5
                })
                .sum::<f64>()
        };
        let mut kept = 0.;
        let mut remainder = 0.;
        for tri in [
            [v(-1., -1.), v(4., -1.), v(4., 4.)],
            [v(-1., -1.), v(4., 4.), v(-1., 4.)],
        ] {
            let (inside, outside) = partition_triangle(&region, tri.clone());
            kept += inside.iter().map(|p| area(p)).sum::<f64>();
            remainder += outside.iter().map(|p| area(p)).sum::<f64>();
            assert!(inside
                .iter()
                .flatten()
                .all(|v| region.contains(v.clip, 1e-8)
                    && (v.values[2] - v.clip[0] - v.clip[1]).abs() < 1e-10));
            let clipped = clip_region(&region, tri);
            assert!(
                (clipped.iter().map(|p| area(p)).sum::<f64>()
                    - inside.iter().map(|p| area(p)).sum::<f64>())
                .abs()
                    < 1e-10
            );
        }
        assert!((kept - 5.).abs() < 1e-10);
        assert!((remainder - 20.).abs() < 1e-10);
        assert!(!region.contains([2., 2., 0.], 0.));
    }
    #[test]
    fn partition_preserves_area_and_affine_inverse_roundtrips() {
        let r = Region {
            frame: IDENTITY,
            parts: Vec::new(),
            polygon: vec![[-1., -1.], [1., -1.], [1., 1.], [-1., 1.]],
        };
        let v = |x, y| Vertex {
            values: vec![x, y, 2.],
            clip: [x, y, 2.],
        };
        let (inside, outside) = partition_triangle(&r, [v(-2., -2.), v(2., -2.), v(2., 2.)]);
        let area = |p: &[Vertex]| {
            (0..p.len())
                .map(|i| {
                    let a = p[i].clip;
                    let b = p[(i + 1) % p.len()].clip;
                    (a[0] * b[1] - b[0] * a[1]) * 0.5
                })
                .sum::<f64>()
        };
        assert!(
            (inside.iter().map(|p| area(p)).sum::<f64>()
                + outside.iter().map(|p| area(p)).sum::<f64>()
                - 8.)
                .abs()
                < 1e-10
        );
        assert!(outside.iter().flatten().all(|v| v.values[2] == 2.));
        let m = [
            0., 2., 0., 0., -3., 0., 0., 0., 0., 0., 4., 0., 6378137., 100., 10., 1.,
        ];
        let p = [0.3, 1.7, -2.];
        let q = point(inverse(m).unwrap(), point(m, p));
        assert!(p.iter().zip(q).all(|(a, b)| (a - b).abs() < 1e-8));
    }
    #[test]
    fn geographic_region_rejects_bad_polygons_and_large_extents() {
        for v in [
            serde_json::json!({"type":"rectangle","bounds":[1.,0.,0.,1.]}),
            serde_json::json!({"type":"rectangle","bounds":[-1.,0.,1.,1.]}),
            serde_json::json!({"type":"Polygon","coordinates":[[[0.,0.],[0.001,0.],[0.,0.001]]]}),
            serde_json::json!({"type":"Polygon","coordinates":[[[0.,0.],[0.001,0.001],[0.,0.001],[0.001,0.],[0.,0.]]]}),
        ] {
            assert!(Region::parse(&v).is_err());
        }
        assert!(Region::parse(
            &serde_json::json!({"type":"rectangle","bounds":[-0.001,-0.001,0.001,0.001]})
        )
        .is_ok());
    }
    #[test]
    fn clips_area_and_interpolates_attributes() {
        let r = Region {
            frame: IDENTITY,
            parts: Vec::new(),
            polygon: vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]],
        };
        let v = |x, y| Vertex {
            values: vec![x, y, x + y],
            clip: [x, y, 0.],
        };
        let out = clip_triangle(&r, [v(-1., 0.), v(1., 0.), v(1., 2.)]);
        assert!(out
            .iter()
            .all(|v| v.clip[0] >= 0. && v.clip[0] <= 1. && v.clip[1] >= 0. && v.clip[1] <= 1.));
        assert!(out
            .iter()
            .all(|v| (v.values[2] - v.clip[0] - v.clip[1]).abs() < 1e-12));
        let area: f64 = (1..out.len() - 1)
            .map(|i| {
                cross(
                    [out[0].clip[0], out[0].clip[1]],
                    [out[i].clip[0], out[i].clip[1]],
                    [out[i + 1].clip[0], out[i + 1].clip[1]],
                ) * 0.5
            })
            .sum();
        assert!((area - 1.).abs() < 1e-12);
        assert!(clip_triangle(&r, [v(-2., 0.), v(-1., 0.), v(-1., 1.)]).is_empty());
    }
}
