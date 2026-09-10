//! CRS / origin helpers (ported from desktop_server geo.py — pragmatic subset).

use regex::Regex;
use serde_json::{json, Value};
use std::sync::OnceLock;

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
    if let Some(arr) = value.as_array() {
        if arr.len() >= 3 {
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
        if parts.len() >= 3 {
            return Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?));
        }
    }
    None
}

pub fn parse_enu_lat_lon(srs: Option<&str>) -> Option<(f64, f64)> {
    let s = srs?;
    let m = enu_re().captures(s)?;
    Some((m[1].parse().ok()?, m[2].parse().ok()?))
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
            let code: i64 = epsg_re().captures(s).and_then(|c| c[1].parse().ok()).unwrap_or(0);
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

    let effective_crs = crs_override
        .clone()
        .or_else(|| scan_srs.clone().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()));

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

pub fn build_tile_config_json(effective: &Value) -> (Option<String>, Vec<String>) {
    let mut notes = Vec::new();
    let mut cfg = serde_json::Map::new();

    let enu = effective.get("enuLatLon");
    if let Some(enu) = enu {
        if effective.get("crsOverride").and_then(|v| v.as_str()).is_some() {
            if let (Some(lon), Some(lat)) = (
                enu.get("lon").and_then(|v| v.as_f64()),
                enu.get("lat").and_then(|v| v.as_f64()),
            ) {
                cfg.insert("x".into(), json!(lon));
                cfg.insert("y".into(), json!(lat));
                notes.push(format!("CLI -c x/y from CRS override ENU lon={lon}, lat={lat}"));
            }
        }
    }

    if let Some(origin) = effective.get("effectiveOrigin") {
        let has_override = effective
            .get("originOverride")
            .map(|v| !v.is_null())
            .unwrap_or(false);
        if has_override {
            if let Some(z) = origin.get("z").and_then(|v| v.as_f64()) {
                cfg.insert("offset".into(), json!(z));
                notes.push(format!("CLI -c offset from origin override z={z}"));
            }
        }
    }

    if effective.get("crsOverride").and_then(|v| v.as_str()).is_some() && enu.is_none() {
        notes.push(
            "CRS override stored in task options; current 3dtile CLI has no EPSG/WKT flag; \
runtime still uses input metadata.xml SRS."
                .into(),
        );
    }

    if cfg.is_empty() {
        (None, notes)
    } else {
        (
            Some(serde_json::to_string(&Value::Object(cfg)).unwrap_or_default()),
            notes,
        )
    }
}
