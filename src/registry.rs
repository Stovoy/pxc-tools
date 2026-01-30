use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};
use regex::Regex;
use serde_json::{Map, Value, json};
use walkdir::WalkDir;

#[derive(Clone, Debug)]
pub struct Registry {
    pub nodes: std::collections::HashMap<String, RegistryNode>,
}

#[derive(Clone, Debug)]
pub struct RegistryNode {
    pub inputs: Vec<RegistryPort>,
    pub outputs: Vec<RegistryPort>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RegistryPort {
    pub name: Option<String>,
    pub ty: Option<String>,
    pub tooltip: Option<String>,
}

pub fn load_registry(path: Option<&Path>) -> Result<Option<Registry>> {
    if let Some(p) = path {
        return Ok(Some(load_registry_file(p)?));
    }

    Ok(Some(embedded_registry_inner()))
}

pub fn embedded_registry() -> Registry {
    embedded_registry_inner()
}

pub(crate) fn embedded_registry_inner() -> Registry {
    let data = include_str!("registry_embedded.json");
    load_registry_from_str(data).expect("embedded registry JSON is invalid")
}

fn load_registry_file(path: &Path) -> Result<Registry> {
    let data = fs::read_to_string(path)?;
    load_registry_from_str(&data)
}

fn load_registry_from_str(data: &str) -> Result<Registry> {
    let v: Value = serde_json::from_str(data)?;
    let mut nodes = std::collections::HashMap::new();
    let obj = v
        .as_object()
        .ok_or_else(|| anyhow!("registry JSON must be an object"))?;
    for (node_name, node_val) in obj {
        let node_obj = match node_val.as_object() {
            Some(v) => v,
            None => continue,
        };
        let inputs = parse_registry_ports(node_obj.get("inputs"));
        let outputs = parse_registry_ports(node_obj.get("outputs"));
        nodes.insert(node_name.clone(), RegistryNode { inputs, outputs });
    }
    Ok(Registry { nodes })
}

fn parse_registry_ports(v: Option<&Value>) -> Vec<RegistryPort> {
    let mut out = Vec::new();
    let arr = match v.and_then(|v| v.as_array()) {
        Some(v) => v,
        None => return out,
    };
    for item in arr {
        if let Some(obj) = item.as_object() {
            out.push(RegistryPort {
                name: obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                ty: obj
                    .get("type")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                tooltip: obj
                    .get("tooltip")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            });
        } else {
            out.push(RegistryPort {
                name: None,
                ty: None,
                tooltip: None,
            });
        }
    }
    out
}

fn load_locale_registry(path: &Path) -> Result<Registry> {
    let data = fs::read_to_string(path)?;
    let v: Value = serde_json::from_str(&data)?;
    let mut nodes = std::collections::HashMap::new();
    let obj = v
        .as_object()
        .ok_or_else(|| anyhow!("locale nodes.json must be an object"))?;
    for (node_name, node_val) in obj {
        let node_obj = match node_val.as_object() {
            Some(v) => v,
            None => continue,
        };
        let inputs = parse_registry_ports(node_obj.get("inputs"));
        let outputs = parse_registry_ports(node_obj.get("outputs"));
        nodes.insert(node_name.clone(), RegistryNode { inputs, outputs });
    }
    Ok(Registry { nodes })
}

pub(crate) fn cmd_registry_build(scripts: &Path, locale: Option<&Path>, out: &Path) -> Result<()> {
    let mut nodes = std::collections::HashMap::new();

    let locale_nodes = if let Some(p) = locale {
        Some(load_locale_registry(p)?)
    } else {
        None
    };

    let node_fn_re = Regex::new(r"function\\s+(Node_[A-Za-z0-9_]+)")?;
    let new_input_re = Regex::new(r"newInput[^,]*,\\s*(new\\s+)?([A-Za-z_][A-Za-z0-9_]*)")?;
    let new_output_re = Regex::new(r"newOutput[^,]*,\\s*(new\\s+)?([A-Za-z_][A-Za-z0-9_]*)")?;
    let name_re = Regex::new(r#"\"([^\"]+)\""#)?;
    let value_type_re = Regex::new(r"VALUE_TYPE\\.([A-Za-z0-9_]+)")?;

    for entry in WalkDir::new(scripts).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|s| s.to_str()) != Some("gml") {
            continue;
        }
        let text = fs::read_to_string(entry.path()).unwrap_or_default();
        let node_name = node_fn_re
            .captures(&text)
            .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
        let node_name = match node_name {
            Some(n) => n,
            None => continue,
        };

        let mut inputs: Vec<Option<RegistryPort>> = Vec::new();
        for cap in new_input_re.captures_iter(&text) {
            let whole = cap.get(0).map(|m| m.as_str()).unwrap_or("");
            let func = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let ty = infer_type_from_fn_with_value(func, whole, &value_type_re);
            let name = name_re
                .captures(whole)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
            let slot = extract_slot(whole, "newInput").unwrap_or(inputs.len());
            if inputs.len() <= slot {
                inputs.resize_with(slot + 1, || None);
            }
            inputs[slot] = Some(RegistryPort {
                name,
                ty,
                tooltip: None,
            });
        }

        let mut outputs: Vec<Option<RegistryPort>> = Vec::new();
        for cap in new_output_re.captures_iter(&text) {
            let whole = cap.get(0).map(|m| m.as_str()).unwrap_or("");
            let func = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let ty = infer_type_from_fn_with_value(func, whole, &value_type_re);
            let name = name_re
                .captures(whole)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));
            let slot = extract_slot(whole, "newOutput").unwrap_or(outputs.len());
            if outputs.len() <= slot {
                outputs.resize_with(slot + 1, || None);
            }
            outputs[slot] = Some(RegistryPort {
                name,
                ty,
                tooltip: None,
            });
        }

        if let Some(locale_reg) = &locale_nodes {
            if let Some(lr) = locale_reg.nodes.get(&node_name) {
                let inputs_compact = compact_ports(inputs);
                let outputs_compact = compact_ports(outputs);
                inputs = expand_ports(merge_registry_ports(&inputs_compact, &lr.inputs));
                outputs = expand_ports(merge_registry_ports(&outputs_compact, &lr.outputs));
            }
        }

        nodes.insert(
            node_name,
            RegistryNode {
                inputs: compact_ports(inputs),
                outputs: compact_ports(outputs),
            },
        );
    }

    let json = registry_to_json(&Registry { nodes });
    fs::write(out, serde_json::to_string_pretty(&json)?)?;
    Ok(())
}

fn merge_registry_ports(a: &[RegistryPort], b: &[RegistryPort]) -> Vec<RegistryPort> {
    let len = a.len().max(b.len());
    let mut out = Vec::new();
    for i in 0..len {
        let pa = a.get(i);
        let pb = b.get(i);
        out.push(RegistryPort {
            name: pa
                .and_then(|p| p.name.clone())
                .or_else(|| pb.and_then(|p| p.name.clone())),
            ty: pa
                .and_then(|p| p.ty.clone())
                .or_else(|| pb.and_then(|p| p.ty.clone())),
            tooltip: pa
                .and_then(|p| p.tooltip.clone())
                .or_else(|| pb.and_then(|p| p.tooltip.clone())),
        });
    }
    out
}

fn registry_to_json(reg: &Registry) -> Value {
    let mut obj = Map::new();
    for (node_name, node) in reg.nodes.iter() {
        let inputs = registry_ports_to_json(&node.inputs);
        let outputs = registry_ports_to_json(&node.outputs);
        obj.insert(
            node_name.clone(),
            json!({
                "inputs": inputs,
                "outputs": outputs
            }),
        );
    }
    Value::Object(obj)
}

fn registry_ports_to_json(ports: &[RegistryPort]) -> Value {
    Value::Array(
        ports
            .iter()
            .map(|p| {
                json!({
                    "name": p.name,
                    "type": p.ty,
                    "tooltip": p.tooltip
                })
            })
            .collect(),
    )
}

fn infer_type_from_fn(func: &str) -> Option<String> {
    let f = func.to_lowercase();
    let f = f
        .trim_start_matches("nodevalue_")
        .trim_start_matches("nodevalue");
    let f = f.trim_start_matches("nodevalue_");
    let f = f.trim_start_matches("nodevalue");
    let f = f.trim_start_matches("__nodevalue_");
    let f = f.trim_start_matches("nodevalue_");
    let f = f.trim_start_matches("nodevalue");

    let ty = if f.contains("surface") {
        "surface"
    } else if f.contains("float") {
        "float"
    } else if f.contains("int") || f.contains("integer") {
        "integer"
    } else if f.contains("bool") {
        "boolean"
    } else if f.contains("color") {
        "color"
    } else if f.contains("text") || f.contains("string") {
        "text"
    } else if f.contains("pathnode") {
        "pathnode"
    } else if f.contains("path") {
        "path"
    } else if f.contains("gradient") {
        "gradient"
    } else if f.contains("vec2") {
        "vec2"
    } else if f.contains("vec3") {
        "vec3"
    } else if f.contains("vec4") {
        "vec4"
    } else if f.contains("range") {
        "range"
    } else if f.contains("matrix") {
        "matrix"
    } else if f.contains("palette") {
        "palette"
    } else if f.contains("rotation") {
        "rotation"
    } else if f.contains("trigger") {
        "trigger"
    } else if f.contains("atlas") {
        "atlas"
    } else if f.contains("mesh") {
        "mesh"
    } else if f.contains("armature") {
        "armature"
    } else if f.contains("buffer") {
        "buffer"
    } else if f.contains("struct") {
        "struct"
    } else if f.contains("particle") {
        "particle"
    } else if f.contains("enum") {
        "enum"
    } else if f.contains("output") {
        "output"
    } else {
        "unknown"
    };
    Some(ty.to_string())
}

fn infer_type_from_fn_with_value(func: &str, snippet: &str, value_re: &Regex) -> Option<String> {
    let func_lower = func.to_lowercase();
    if func_lower == "nodevalue"
        || func_lower == "nodevalue_output"
        || func_lower == "nodevalue_output".to_string()
    {
        if let Some(cap) = value_re.captures(snippet) {
            if let Some(m) = cap.get(1) {
                return Some(m.as_str().to_lowercase());
            }
        }
    }
    infer_type_from_fn(func)
}

fn extract_slot(snippet: &str, key: &str) -> Option<usize> {
    let idx = snippet.find(key)?;
    let mut found = false;
    let mut num = String::new();
    for ch in snippet[idx + key.len()..].chars() {
        if ch.is_ascii_digit() {
            found = true;
            num.push(ch);
        } else if found {
            break;
        }
    }
    if num.is_empty() {
        None
    } else {
        num.parse::<usize>().ok()
    }
}

fn compact_ports(ports: Vec<Option<RegistryPort>>) -> Vec<RegistryPort> {
    ports
        .into_iter()
        .map(|p| {
            p.unwrap_or(RegistryPort {
                name: None,
                ty: None,
                tooltip: None,
            })
        })
        .collect()
}

fn expand_ports(ports: Vec<RegistryPort>) -> Vec<Option<RegistryPort>> {
    ports.into_iter().map(Some).collect()
}
