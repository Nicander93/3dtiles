//! CRS / origin helpers (ported from desktop_server geo.py — pragmatic subset).

use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;

pub fn cartographic_to_ecef(lon: f64, lat: f64, height: f64) -> [f64; 3] {
    let (s, c) = lat.to_radians().sin_cos();
    let (sl, cl) = lon.to_radians().sin_cos();
    let n = 6378137. / (1. - 6.6943799901413165e-3 * s * s).sqrt();
    [
        (n + height) * c * cl,
        (n + height) * c * sl,
        (n * (1. - 6.6943799901413165e-3) + height) * s,
    ]
}
pub fn enu_to_ecef(lon: f64, lat: f64, height: f64) -> [f64; 16] {
    let (s, c) = lat.to_radians().sin_cos();
    let (sl, cl) = lon.to_radians().sin_cos();
    let p = cartographic_to_ecef(lon, lat, height);
    [
        -sl,
        cl,
        0.,
        0.,
        -s * cl,
        -s * sl,
        c,
        0.,
        c * cl,
        c * sl,
        s,
        0.,
        p[0],
        p[1],
        p[2],
        1.,
    ]
}

fn enu_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*ENU\s*:\s*([+-]?\d+(?:\.\d+)?)\s*,\s*([+-]?\d+(?:\.\d+)?)\s*$")
            .unwrap()
    })
}

fn epsg_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^\s*EPSG\s*:\s*(\d+)\s*$").unwrap())
}

pub fn parse_origin_xyz(value: &Value) -> Option<(f64, f64, f64)> {
    let p = parse_origin_components(value)?;
    [p.0, p.1, p.2].iter().all(|v| v.is_finite()).then_some(p)
}
fn parse_origin_components(value: &Value) -> Option<(f64, f64, f64)> {
    if let Some(arr) = value.as_array() {
        if arr.len() == 3 {
            return Some((arr[0].as_f64()?, arr[1].as_f64()?, arr[2].as_f64()?));
        }
    }
    if let Some(obj) = value.as_object() {
        let x = obj.get("x")?.as_f64()?;
        let y = obj.get("y")?.as_f64()?;
        let z = obj.get("z")?.as_f64()?;
        return Some((x, y, z));
    }
    if let Some(text) = value.as_str() {
        let parts: Vec<&str> = text
            .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .collect();
        if parts.len() == 3 {
            return Some((
                parts[0].parse().ok()?,
                parts[1].parse().ok()?,
                parts[2].parse().ok()?,
            ));
        }
    }
    None
}

pub fn parse_enu_lat_lon(srs: Option<&str>) -> Option<(f64, f64)> {
    let s = srs?;
    let m = enu_re().captures(s)?;
    let lat: f64 = m[1].parse().ok()?;
    let lon: f64 = m[2].parse().ok()?;
    (lat.is_finite() && lon.is_finite() && lat.abs() <= 90. && lon.abs() <= 180.)
        .then_some((lat, lon))
}

/// The installed OSGB converter reads CRS/origin from metadata, not a complete override.
/// Reject changes it cannot honor instead of reporting them as effective coordinates.
pub fn validate_osgb_geo(scan: &Value, options: &Value) -> Result<(), String> {
    let geo = geo_opts(options);
    for value in [&geo, options.get("convert").unwrap_or(&Value::Null)] {
        if ["x", "y", "offset"]
            .iter()
            .any(|key| value.get(key).is_some())
        {
            return Err("unsupported OSGB coordinate override: use metadata.xml SRS/SRSOrigin instead of x/y/offset".into());
        }
    }
    if let Some(crs) = geo
        .get("crs")
        .or_else(|| geo.get("crsOverride"))
        .or_else(|| options.get("crsOverride"))
    {
        if !crs.is_string() {
            return Err("invalid OSGB CRS: expected a string".into());
        }
    }
    let effective = resolve_effective_geo(scan, options);
    if let Some(crs) = effective["effectiveCrs"].as_str() {
        if crs.trim().to_uppercase().starts_with("ENU") && parse_enu_lat_lon(Some(crs)).is_none() {
            return Err(
                "invalid OSGB ENU coordinates: use ENU:latitude,longitude within ±90°,±180°".into(),
            );
        }
    }
    if let Some(crs) = effective["crsOverride"].as_str() {
        let source = effective["scanSrs"].as_str().unwrap_or("").trim();
        let same = crs == source
            || (parse_enu_lat_lon(Some(crs)).is_some()
                && parse_enu_lat_lon(Some(crs)) == parse_enu_lat_lon(Some(source)))
            || (parse_epsg_code(Some(crs)).is_some()
                && parse_epsg_code(Some(crs)) == parse_epsg_code(Some(source)));
        if !same {
            return Err("unsupported OSGB coordinate override: installed converter uses metadata.xml SRS; update source metadata instead".into());
        }
    }
    let origin = geo
        .get("origin")
        .or_else(|| geo.get("originOverride"))
        .or_else(|| options.get("originOverride"));
    let fields = ["originX", "originY", "originZ"];
    let has_fields = fields.iter().any(|k| geo.get(k).is_some());
    let requested = if has_fields {
        let a: Option<Vec<f64>> = fields
            .iter()
            .map(|k| geo.get(k)?.as_f64().filter(|v| v.is_finite()))
            .collect();
        let a = a.ok_or("invalid OSGB origin: all X/Y/Z values must be finite")?;
        Some((a[0], a[1], a[2]))
    } else if let Some(v) = origin {
        Some(
            parse_origin_xyz(v)
                .ok_or("invalid OSGB origin: exactly three finite X/Y/Z values required")?,
        )
    } else {
        None
    };
    if let Some(requested) = requested {
        let source = effective["scanOrigin"]
            .as_str()
            .and_then(|s| parse_origin_xyz(&json!(s)));
        if source != Some(requested) {
            return Err("unsupported OSGB origin override: installed converter uses metadata.xml SRSOrigin; update source metadata instead".into());
        }
    }
    if let Some(config) = options.pointer("/convert/config").and_then(Value::as_str) {
        let config: Value =
            serde_json::from_str(config).map_err(|_| "invalid converter config JSON")?;
        if ["x", "y", "offset"].iter().any(|k| config.get(k).is_some()) {
            return Err("unsupported OSGB coordinate override in convert.config; use metadata.xml SRS/SRSOrigin".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod coordinate_tests {
    use super::*;
    #[test]
    fn enu_and_origin_reject_invalid_coordinates() {
        assert_eq!(parse_enu_lat_lon(Some("ENU:-35,-73")), Some((-35., -73.)));
        for s in ["ENU:91,0", "ENU:0,181", "ENU:NaN,0"] {
            assert!(parse_enu_lat_lon(Some(s)).is_none());
        }
        for v in [json!("1,2,NaN"), json!("1,2,3,4"), json!([1, 2])] {
            assert!(parse_origin_xyz(&v).is_none());
        }
    }
    #[test]
    fn osgb_metadata_is_used_and_unsupported_overrides_fail() {
        let scan = json!({"metadata":{"srs":"EPSG:4547","srsOrigin":"1,2,3"}});
        assert!(validate_osgb_geo(&scan, &json!({})).is_ok());
        assert!(
            validate_osgb_geo(&scan, &json!({"geo":{"crs":"epsg:4547","origin":[1,2,3]}})).is_ok()
        );
        for options in [
            json!({"geo":{"crs":"EPSG:3857"}}),
            json!({"geo":{"origin":[2,2,3]}}),
            json!({"originX":1}),
            json!({"convert":{"offset":5}}),
            json!({"geo":{"crs":12}}),
            json!({"convert":{"config":"{\"x\":117}"}}),
        ] {
            assert!(validate_osgb_geo(&scan, &options).is_err(), "{options}");
        }
    }
    #[test]
    fn ecef_reference_points_and_enu_axes() {
        assert_eq!(cartographic_to_ecef(0., 0., 10.), [6378147., 0., 0.]);
        let pole = cartographic_to_ecef(0., 90., 0.);
        assert!(pole[0].abs() < 1e-8 && (pole[2] - 6356752.314245179).abs() < 1e-8);
        let frame = enu_to_ecef(0., 0., 0.);
        assert_eq!(&frame[0..3], &[0., 1., 0.]);
        assert_eq!(&frame[4..7], &[0., 0., 1.]);
        assert_eq!(&frame[8..11], &[1., 0., 0.]);
    }
}

pub fn parse_epsg_code(srs: Option<&str>) -> Option<i64> {
    let s = srs?;
    let m = epsg_re().captures(s)?;
    m[1].parse().ok()
}

pub fn unit_hint(srs: Option<&str>) -> String {
    match srs {
        None => "未知单位（缺 SRS）".into(),
        Some(s) if enu_re().is_match(s) => {
            "ENU 局部坐标：米（东/北/天）；地理原点为经纬度（度）".into()
        }
        Some(s) if epsg_re().is_match(s) => {
            let code: i64 = epsg_re()
                .captures(s)
                .and_then(|c| c[1].parse().ok())
                .unwrap_or(0);
            if matches!(code, 4326 | 4490 | 4610 | 4214) {
                format!("EPSG:{code} 地理坐标：度（注意轴序）；高程另计")
            } else {
                format!("EPSG:{code} 多为投影米制；请确认分带/假东移，高程另计")
            }
        }
        Some(s) if s.to_uppercase().contains("GEOGCS") || s.to_uppercase().contains("PROJCS") => {
            "WKT：按定义单位；请自行确认轴序与高程基准".into()
        }
        _ => "自定义 CRS：请确认单位与轴序；高程基准独立".into(),
    }
}

fn geo_opts(options: &Value) -> Value {
    if let Some(geo) = options.get("geo") {
        if geo.is_object() {
            return geo.clone();
        }
    }
    let mut out = serde_json::Map::new();
    for k in [
        "crs",
        "crsOverride",
        "origin",
        "originOverride",
        "originX",
        "originY",
        "originZ",
        "geographicExport",
    ] {
        if let Some(v) = options.get(k) {
            if !v.is_null() {
                out.insert(k.into(), v.clone());
            }
        }
    }
    if let Some(conv) = options.get("convert").and_then(|v| v.as_object()) {
        for k in ["crs", "origin", "x", "y", "offset", "geographicExport"] {
            if out.contains_key(k) {
                continue;
            }
            if let Some(v) = conv.get(k) {
                if !v.is_null() {
                    out.insert(k.into(), v.clone());
                }
            }
        }
    }
    Value::Object(out)
}

pub fn resolve_effective_geo(scan: &Value, options: &Value) -> Value {
    let geo = geo_opts(options);
    let meta = scan.get("metadata").cloned().unwrap_or(json!({}));
    let summary = scan.get("summary").cloned().unwrap_or(json!({}));

    let scan_srs = meta
        .get("srs")
        .and_then(|v| v.as_str())
        .or_else(|| summary.get("srs").and_then(|v| v.as_str()))
        .map(|s| s.to_string());
    let scan_origin = meta
        .get("srsOrigin")
        .and_then(|v| v.as_str())
        .or_else(|| summary.get("srsOrigin").and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    let crs_override = geo
        .get("crs")
        .or_else(|| geo.get("crsOverride"))
        .or_else(|| options.get("crsOverride"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let mut origin_override = None;
    if let (Some(x), Some(y), Some(z)) = (
        geo.get("originX").and_then(|v| v.as_f64()),
        geo.get("originY").and_then(|v| v.as_f64()),
        geo.get("originZ").and_then(|v| v.as_f64()),
    ) {
        origin_override = Some((x, y, z));
    }
    if origin_override.is_none() {
        if let Some(v) = geo
            .get("origin")
            .or_else(|| geo.get("originOverride"))
            .or_else(|| options.get("originOverride"))
        {
            origin_override = parse_origin_xyz(v);
        }
    }

    let effective_crs = crs_override.clone().or_else(|| {
        scan_srs
            .clone()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    });

    let effective_origin = if let Some((x, y, z)) = origin_override {
        Some(json!({
            "x": x, "y": y, "z": z,
            "source": "override",
            "text": format!("{x},{y},{z}"),
        }))
    } else if let Some(ref so) = scan_origin {
        let parsed = parse_origin_xyz(&json!(so));
        Some(json!({
            "x": parsed.map(|p| p.0),
            "y": parsed.map(|p| p.1),
            "z": parsed.map(|p| p.2),
            "source": "metadata",
            "text": so,
        }))
    } else {
        None
    };

    let geographic = geo
        .get("geographicExport")
        .or_else(|| geo.get("requireCrs"))
        .or_else(|| options.get("geographicExport"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let enu = parse_enu_lat_lon(effective_crs.as_deref());
    let ep = parse_epsg_code(effective_crs.as_deref());

    json!({
        "scanSrs": scan_srs,
        "scanOrigin": scan_origin,
        "crsOverride": crs_override,
        "originOverride": origin_override.map(|(x,y,z)| json!({"x":x,"y":y,"z":z})),
        "effectiveCrs": effective_crs,
        "effectiveOrigin": effective_origin,
        "unitHint": unit_hint(effective_crs.as_deref()),
        "geographicExport": geographic,
        "enuLatLon": enu.map(|(lat, lon)| json!({"lat": lat, "lon": lon})),
        "epsg": ep,
        "hasCrs": effective_crs.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
    })
}

pub fn missing_crs_message(effective: &Value) -> Option<String> {
    if !effective
        .get("geographicExport")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return None;
    }
    if effective
        .get("hasCrs")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return None;
    }
    Some(
        "缺参数阻止地理导出：扫描未得到 SRS，且未填写 CRS 覆盖。\
本地预览/非地理转换可关闭「地理导出」；\
地理定位请在 metadata.xml 提供 SRS（ENU:/EPSG:/WKT）或在转换页填写 CRS 覆盖。"
            .into(),
    )
}

pub fn build_tile_config_json(_effective: &Value) -> (Option<String>, Vec<String>) {
    // OSGB uses its metadata; matching overrides are a no-op, never extra offsets.
    (None, Vec::new())
}
