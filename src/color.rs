use serde_json::Value;

use crate::pxc::PxcFile;

fn rgb_to_hsl(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let mut h = 0.0;
    let l = (max + min) / 2.0;

    if (max - min).abs() < f64::EPSILON {
        return (0.0, 0.0, l);
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    if (max - r).abs() < f64::EPSILON {
        h = (g - b) / d + if g < b { 6.0 } else { 0.0 };
    } else if (max - g).abs() < f64::EPSILON {
        h = (b - r) / d + 2.0;
    } else if (max - b).abs() < f64::EPSILON {
        h = (r - g) / d + 4.0;
    }
    h /= 6.0;
    (h, s, l)
}

fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (l, l, l);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
    (r, g, b)
}

pub(crate) fn color_from_value(v: &Value) -> Option<u32> {
    if let Value::Number(n) = v {
        if let Some(u) = n.as_u64() {
            if u <= u32::MAX as u64 {
                let c = u as u32;
                if c & 0xFF00_0000 != 0 {
                    return Some(c);
                }
            }
        } else if let Some(i) = n.as_i64() {
            if i >= 0 && i <= u32::MAX as i64 {
                let c = i as u32;
                if c & 0xFF00_0000 != 0 {
                    return Some(c);
                }
            }
        } else if let Some(f) = n.as_f64() {
            if f.is_finite() {
                let r = f.round();
                if (r - f).abs() < 0.0001 && r >= 0.0 && r <= u32::MAX as f64 {
                    let c = r as u32;
                    if c & 0xFF00_0000 != 0 {
                        return Some(c);
                    }
                }
            }
        }
    }
    None
}

pub(crate) fn color_from_rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((a as u32) << 24) | ((b as u32) << 16) | ((g as u32) << 8) | (r as u32)
}

pub(crate) fn default_gradient_value() -> Value {
    let s = r#"{"type":0,"keys":[{"time":0,"value":4294967295}]}"#;
    Value::String(s.to_string())
}

pub(crate) fn gradient_value_from_keys(keys: &[(f64, u32)], interp: i32) -> Value {
    let mut arr = Vec::with_capacity(keys.len());
    for (t, c) in keys {
        arr.push(serde_json::json!({"time": t, "value": *c}));
    }
    let obj = serde_json::json!({"type": interp, "keys": arr});
    Value::String(obj.to_string())
}

fn hue_set_color(color: u32, hue_deg: f64) -> u32 {
    let a = ((color >> 24) & 0xFF) as u8;
    let r = (color & 0xFF) as f64 / 255.0;
    let g = ((color >> 8) & 0xFF) as f64 / 255.0;
    let b = ((color >> 16) & 0xFF) as f64 / 255.0;
    let (_h, s, l) = rgb_to_hsl(r, g, b);
    let mut s2 = s;
    if s2 < 0.15 {
        s2 = 0.25;
    }
    let h = (hue_deg / 360.0).rem_euclid(1.0);
    let (r2, g2, b2) = hsl_to_rgb(h, s2, l);
    let r8 = (r2.clamp(0.0, 1.0) * 255.0).round() as u32;
    let g8 = (g2.clamp(0.0, 1.0) * 255.0).round() as u32;
    let b8 = (b2.clamp(0.0, 1.0) * 255.0).round() as u32;
    (a as u32) << 24 | (b8 << 16) | (g8 << 8) | r8
}

fn looks_like_color_array(arr: &[Value]) -> bool {
    if arr.is_empty() {
        return false;
    }
    let mut any = false;
    for v in arr {
        if !matches!(v, Value::Number(_)) {
            return false;
        }
        if color_from_value(v).is_some() {
            any = true;
        }
    }
    any
}

fn key_is_colorish(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    k.contains("color") || k.contains("colour")
}

fn hue_set_value(value: &mut Value, key_name: Option<&str>, hue_deg: f64) -> usize {
    match value {
        Value::Object(map) => {
            let mut changed = 0usize;
            let keys: Vec<String> = map.keys().cloned().collect();
            for k in keys {
                if let Some(v) = map.get_mut(&k) {
                    changed += hue_set_value(v, Some(&k), hue_deg);
                }
            }
            changed
        }
        Value::Array(arr) => {
            if looks_like_color_array(arr) {
                let mut changed = 0usize;
                for v in arr.iter_mut() {
                    if let Some(c) = color_from_value(v) {
                        let out = hue_set_color(c, hue_deg);
                        *v = Value::Number(out.into());
                        changed += 1;
                    }
                }
                return changed;
            }
            let mut changed = 0usize;
            for v in arr.iter_mut() {
                changed += hue_set_value(v, None, hue_deg);
            }
            changed
        }
        Value::String(s) => {
            let Ok(mut v) = serde_json::from_str::<Value>(s) else {
                return 0;
            };
            let mut changed = 0usize;
            if let Some(keys) = v.get_mut("keys").and_then(|v| v.as_array_mut()) {
                for k in keys.iter_mut() {
                    if let Some(obj) = k.as_object_mut() {
                        if let Some(val) = obj.get_mut("value") {
                            if let Some(c) = color_from_value(val) {
                                let out = hue_set_color(c, hue_deg);
                                *val = Value::Number(out.into());
                                changed += 1;
                            }
                        }
                    }
                }
            }
            if changed > 0 {
                if let Ok(ns) = serde_json::to_string(&v) {
                    *s = ns;
                }
            }
            changed
        }
        Value::Number(_) => {
            if let Some(k) = key_name {
                if k == "value" || k == "d" || key_is_colorish(k) {
                    if let Some(c) = color_from_value(value) {
                        let out = hue_set_color(c, hue_deg);
                        *value = Value::Number(out.into());
                        return 1;
                    }
                }
            } else if let Some(c) = color_from_value(value) {
                let out = hue_set_color(c, hue_deg);
                *value = Value::Number(out.into());
                return 1;
            }
            0
        }
        _ => 0,
    }
}

pub fn hue_set_pxc(pxc: &mut PxcFile, hue_deg: f64) -> usize {
    let mut changed = hue_set_value(&mut pxc.json, None, hue_deg);
    if let Some(nodes) = pxc.json.get_mut("nodes").and_then(|v| v.as_array_mut()) {
        for node in nodes.iter_mut() {
            if let Some(inputs) = node.get_mut("inputs").and_then(|v| v.as_array_mut()) {
                for input in inputs.iter_mut() {
                    if let Some(obj) = input.as_object_mut() {
                        if let Some(r) = obj.get_mut("r") {
                            if let Some(r_obj) = r.as_object_mut() {
                                if let Some(d) = r_obj.get_mut("d") {
                                    changed += hue_set_value(d, Some("d"), hue_deg);
                                }
                            }
                        }
                        if let Some(a) = obj.get_mut("animators") {
                            changed += hue_set_value(a, None, hue_deg);
                        }
                    }
                }
            }
        }
    }
    changed
}
