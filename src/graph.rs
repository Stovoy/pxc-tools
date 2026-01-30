use std::path::Path;

use anyhow::{Result, anyhow};
use clap::ValueEnum;
use serde_json::{Map, Value, json};

use crate::ids::{short_for_id, short_id};
use crate::pxc::{PxcFile, read_pxc};
use crate::registry::{Registry, load_registry};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum GraphFormat {
    Mermaid,
    Dot,
    Json,
    Summary,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum GraphMode {
    Summary,
    Compact,
    Full,
}

pub(crate) fn cmd_graph(
    path: &Path,
    format: GraphFormat,
    mode: GraphMode,
    pretty: bool,
    include_id_map: bool,
    include_ids: bool,
    include_pos: bool,
    json_inputs: bool,
    full_ids: bool,
    include_edges: bool,
    registry_path: Option<&Path>,
) -> Result<()> {
    let pxc = read_pxc(path)?;
    let nodes = pxc
        .json
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("no nodes array found"))?;

    let registry = load_registry(registry_path)?;

    let mut node_map = Map::new();
    let mut id_map: Map<String, Value> = Map::new();
    let mut id_list: Vec<String> = Vec::new();
    for node in nodes {
        if let Some(id) = node.get("id").and_then(|v| v.as_str()) {
            let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let typ = node.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let label = format!("{}\\n{}", name, typ).trim().to_string();
            id_list.push(id.to_string());
            node_map.insert(
                id.to_string(),
                json!({ "id": id, "name": name, "type": typ, "label": label }),
            );
        }
    }

    if !full_ids {
        for (i, id) in id_list.iter().enumerate() {
            let short = short_id(i);
            id_map.insert(short, Value::String(id.clone()));
        }
    }

    let mut edges = Vec::new();
    let mut outputs_used: std::collections::HashMap<String, std::collections::HashSet<usize>> =
        std::collections::HashMap::new();
    for node in nodes {
        let to_id = match node.get("id").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => continue,
        };
        let inputs = match node.get("inputs").and_then(|v| v.as_array()) {
            Some(v) => v,
            None => continue,
        };
        for (idx, input) in inputs.iter().enumerate() {
            let from_node = input.get("from_node").and_then(|v| v.as_str());
            let from_index = input
                .get("from_index")
                .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)));
            if let (Some(from_node), Some(from_index)) = (from_node, from_index) {
                outputs_used
                    .entry(from_node.to_string())
                    .or_default()
                    .insert(from_index as usize);
                let from_tag = input
                    .get("from_tag")
                    .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)));
                let mut edge = json!({
                    "f": from_node,
                    "fo": from_index,
                    "t": to_id,
                    "ti": idx,
                });
                if let Some(tag) = from_tag {
                    edge["tg"] = Value::Number(tag.into());
                }
                if json_inputs {
                    edge["input"] = input.clone();
                }
                edges.push(edge);
            }
        }
    }

    match format {
        GraphFormat::Json => {
            let mut out_nodes = Map::new();
            for (id, node) in node_map.iter() {
                let key = if full_ids {
                    id.clone()
                } else {
                    short_for_id(&id_map, id).unwrap_or_else(|| id.clone())
                };
                out_nodes.insert(
                    key,
                    build_node_dump(
                        node,
                        nodes,
                        id,
                        mode,
                        include_ids,
                        include_pos,
                        json_inputs,
                        full_ids,
                        &id_map,
                        registry.as_ref(),
                        &outputs_used,
                    ),
                );
            }
            let mut out_edges = Vec::new();
            for edge in edges.iter() {
                let from = edge.get("f").and_then(|v| v.as_str()).unwrap_or("");
                let to = edge.get("t").and_then(|v| v.as_str()).unwrap_or("");
                let from_key = if full_ids {
                    from.to_string()
                } else {
                    short_for_id(&id_map, from).unwrap_or_else(|| from.to_string())
                };
                let to_key = if full_ids {
                    to.to_string()
                } else {
                    short_for_id(&id_map, to).unwrap_or_else(|| to.to_string())
                };
                let mut out_edge = edge.clone();
                out_edge["f"] = Value::String(from_key);
                out_edge["t"] = Value::String(to_key);
                out_edges.push(out_edge);
            }
            let mut out = Map::new();
            if include_id_map {
                if full_ids {
                    out.insert("m".to_string(), Value::Null);
                } else {
                    out.insert("m".to_string(), Value::Object(id_map.clone()));
                }
            }
            out.insert(
                "l".to_string(),
                json!({
                    "n":"nodes","e":"edges","m":"id_map","l":"legend",
                    "node.n":"name","node.t":"type","node.i":"inputs","node.o":"outputs","node.p":"pos","node.id":"full id",
                    "io.s":"slot","io.n":"name","io.t":"type","io.v":"value","io.a":"attri","io.c":"connection",
                    "conn.f":"from node","conn.fo":"from output","conn.tg":"tag",
                    "anim.an":"animated","anim.k":"key count","anim.ad":"animation data (full)"
                }),
            );
            out.insert("n".to_string(), Value::Object(out_nodes));
            if include_edges {
                out.insert("e".to_string(), Value::Array(out_edges));
            }
            let out = Value::Object(out);
            if pretty {
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("{}", serde_json::to_string(&out)?);
            }
        }
        GraphFormat::Summary => {
            let mut summary = Vec::new();
            for node in nodes {
                let id = match node.get("id").and_then(|v| v.as_str()) {
                    Some(v) => v,
                    None => continue,
                };
                let short = if full_ids {
                    id.to_string()
                } else {
                    short_for_id(&id_map, id).unwrap_or_else(|| id.to_string())
                };
                let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let typ = node.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let x = node.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let y = node.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                summary.push(format!("{} {} ({}) @({}, {})", short, name, typ, x, y));
            }
            for line in summary {
                println!("{}", line);
            }
            println!("\nConnections:");
            for edge in edges {
                let from = edge.get("f").and_then(|v| v.as_str()).unwrap_or("");
                let to = edge.get("t").and_then(|v| v.as_str()).unwrap_or("");
                let from_id = if full_ids {
                    from.to_string()
                } else {
                    short_for_id(&id_map, from).unwrap_or_else(|| from.to_string())
                };
                let to_id = if full_ids {
                    to.to_string()
                } else {
                    short_for_id(&id_map, to).unwrap_or_else(|| to.to_string())
                };
                let from_index = edge.get("fo").and_then(|v| v.as_i64()).unwrap_or(-1);
                let to_input = edge.get("ti").and_then(|v| v.as_u64()).unwrap_or(0);
                println!(
                    "{}: out{} -> {}: in{}",
                    from_id, from_index, to_id, to_input
                );
            }
        }
        GraphFormat::Mermaid => {
            if !full_ids {
                println!("%% id_map (short -> full)");
                for (short, full) in id_map.iter() {
                    let full = full.as_str().unwrap_or("");
                    println!("%% {} = {}", short, full);
                }
            }
            println!("graph TD");
            for (id, node) in node_map.iter() {
                let node_id = if full_ids {
                    id.clone()
                } else {
                    short_for_id(&id_map, id).unwrap_or_else(|| id.clone())
                };
                let label = node.get("label").and_then(|v| v.as_str()).unwrap_or("");
                println!("  {}[\"{}\"]", mermaid_id(&node_id), escape_label(label));
            }
            for edge in edges {
                let from = edge.get("f").and_then(|v| v.as_str()).unwrap_or("");
                let to = edge.get("t").and_then(|v| v.as_str()).unwrap_or("");
                let from_id = if full_ids {
                    from.to_string()
                } else {
                    short_for_id(&id_map, from).unwrap_or_else(|| from.to_string())
                };
                let to_id = if full_ids {
                    to.to_string()
                } else {
                    short_for_id(&id_map, to).unwrap_or_else(|| to.to_string())
                };
                let from_index = edge.get("fo").and_then(|v| v.as_i64()).unwrap_or(-1);
                let to_input = edge.get("ti").and_then(|v| v.as_u64()).unwrap_or(0);
                let mut label = format!("out{} -> in{}", from_index, to_input);
                if let Some(tag) = edge.get("tg").and_then(|v| v.as_i64()) {
                    label.push_str(&format!(" (tag {})", tag));
                }
                println!(
                    "  {} -->|\"{}\"| {}",
                    mermaid_id(&from_id),
                    escape_label(&label),
                    mermaid_id(&to_id)
                );
            }
        }
        GraphFormat::Dot => {
            println!("digraph pxc {{");
            println!("  rankdir=LR;");
            if !full_ids {
                println!("  // id_map (short -> full)");
                for (short, full) in id_map.iter() {
                    let full = full.as_str().unwrap_or("");
                    println!("  // {} = {}", short, full);
                }
            }
            for (id, node) in node_map.iter() {
                let node_id = if full_ids {
                    id.clone()
                } else {
                    short_for_id(&id_map, id).unwrap_or_else(|| id.clone())
                };
                let label = node.get("label").and_then(|v| v.as_str()).unwrap_or("");
                println!(
                    "  \"{}\" [label=\"{}\"];\n",
                    escape_dot(&node_id),
                    escape_label(label)
                );
            }
            for edge in edges {
                let from = edge.get("f").and_then(|v| v.as_str()).unwrap_or("");
                let to = edge.get("t").and_then(|v| v.as_str()).unwrap_or("");
                let from_id = if full_ids {
                    from.to_string()
                } else {
                    short_for_id(&id_map, from).unwrap_or_else(|| from.to_string())
                };
                let to_id = if full_ids {
                    to.to_string()
                } else {
                    short_for_id(&id_map, to).unwrap_or_else(|| to.to_string())
                };
                let from_index = edge.get("fo").and_then(|v| v.as_i64()).unwrap_or(-1);
                let to_input = edge.get("ti").and_then(|v| v.as_u64()).unwrap_or(0);
                let mut label = format!("out{} -> in{}", from_index, to_input);
                if let Some(tag) = edge.get("tg").and_then(|v| v.as_i64()) {
                    label.push_str(&format!(" (tag {})", tag));
                }
                println!(
                    "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
                    escape_dot(&from_id),
                    escape_dot(&to_id),
                    escape_label(&label)
                );
            }
            println!("}}");
        }
    }

    Ok(())
}

pub(crate) fn graph_json_from_pxc(
    pxc: &PxcFile,
    mode: GraphMode,
    include_id_map: bool,
    include_ids: bool,
    include_pos: bool,
    json_inputs: bool,
    full_ids: bool,
    include_edges: bool,
    registry_path: Option<&Path>,
) -> Result<Value> {
    let nodes = pxc
        .json
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("no nodes array found"))?;

    let registry = load_registry(registry_path)?;

    let mut node_map = Map::new();
    let mut id_map: Map<String, Value> = Map::new();
    let mut id_list: Vec<String> = Vec::new();
    for node in nodes {
        if let Some(id) = node.get("id").and_then(|v| v.as_str()) {
            let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let typ = node.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let label = format!("{}\n{}", name, typ).trim().to_string();
            id_list.push(id.to_string());
            node_map.insert(
                id.to_string(),
                json!({ "id": id, "name": name, "type": typ, "label": label }),
            );
        }
    }

    if !full_ids {
        for (i, id) in id_list.iter().enumerate() {
            let short = short_id(i);
            id_map.insert(short, Value::String(id.clone()));
        }
    }

    let mut edges = Vec::new();
    let mut outputs_used: std::collections::HashMap<String, std::collections::HashSet<usize>> =
        std::collections::HashMap::new();
    for node in nodes {
        let to_id = match node.get("id").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => continue,
        };
        let inputs = match node.get("inputs").and_then(|v| v.as_array()) {
            Some(v) => v,
            None => continue,
        };
        for (idx, input) in inputs.iter().enumerate() {
            let from_node = input.get("from_node").and_then(|v| v.as_str());
            let from_index = input
                .get("from_index")
                .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)));
            if let (Some(from_node), Some(from_index)) = (from_node, from_index) {
                outputs_used
                    .entry(from_node.to_string())
                    .or_default()
                    .insert(from_index as usize);
                let from_tag = input
                    .get("from_tag")
                    .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)));
                let mut edge = json!({
                    "f": from_node,
                    "fo": from_index,
                    "t": to_id,
                    "ti": idx,
                });
                if let Some(tag) = from_tag {
                    edge["tg"] = Value::Number(tag.into());
                }
                if json_inputs {
                    edge["input"] = input.clone();
                }
                edges.push(edge);
            }
        }
    }

    let mut out_nodes = Map::new();
    for (id, node) in node_map.iter() {
        let key = if full_ids {
            id.clone()
        } else {
            short_for_id(&id_map, id).unwrap_or_else(|| id.clone())
        };
        out_nodes.insert(
            key,
            build_node_dump(
                node,
                nodes,
                id,
                mode,
                include_ids,
                include_pos,
                json_inputs,
                full_ids,
                &id_map,
                registry.as_ref(),
                &outputs_used,
            ),
        );
    }

    let mut out_edges = Vec::new();
    for edge in edges.iter() {
        let from = edge.get("f").and_then(|v| v.as_str()).unwrap_or("");
        let to = edge.get("t").and_then(|v| v.as_str()).unwrap_or("");
        let from_key = if full_ids {
            from.to_string()
        } else {
            short_for_id(&id_map, from).unwrap_or_else(|| from.to_string())
        };
        let to_key = if full_ids {
            to.to_string()
        } else {
            short_for_id(&id_map, to).unwrap_or_else(|| to.to_string())
        };
        let mut out_edge = edge.clone();
        out_edge["f"] = Value::String(from_key);
        out_edge["t"] = Value::String(to_key);
        out_edges.push(out_edge);
    }

    let mut out = Map::new();
    if include_id_map {
        if full_ids {
            out.insert("m".to_string(), Value::Null);
        } else {
            out.insert("m".to_string(), Value::Object(id_map));
        }
    }
    out.insert(
        "l".to_string(),
        json!({
            "n":"nodes","e":"edges","m":"id_map","l":"legend",
            "node.n":"name","node.t":"type","node.i":"inputs","node.o":"outputs","node.p":"pos","node.id":"full id",
            "io.s":"slot","io.n":"name","io.t":"type","io.v":"value","io.a":"attri","io.c":"connection",
            "conn.f":"from node","conn.fo":"from output","conn.tg":"tag",
            "anim.an":"animated","anim.k":"key count","anim.ad":"animation data (full)"
        }),
    );
    out.insert("n".to_string(), Value::Object(out_nodes));
    if include_edges {
        out.insert("e".to_string(), Value::Array(out_edges));
    }
    Ok(Value::Object(out))
}

pub fn graph_json(
    path: &Path,
    mode: GraphMode,
    include_id_map: bool,
    include_ids: bool,
    include_pos: bool,
    json_inputs: bool,
    full_ids: bool,
    include_edges: bool,
    registry_path: Option<&Path>,
) -> Result<Value> {
    let pxc = read_pxc(path)?;
    graph_json_from_pxc(
        &pxc,
        mode,
        include_id_map,
        include_ids,
        include_pos,
        json_inputs,
        full_ids,
        include_edges,
        registry_path,
    )
}

fn escape_label(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\"', "\\\"")
}

fn escape_dot(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\"', "\\\"")
}

fn mermaid_id(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for ch in id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() { "_".to_string() } else { out }
}

fn build_node_dump(
    node_meta: &Value,
    nodes: &[Value],
    id: &str,
    mode: GraphMode,
    include_ids: bool,
    include_pos: bool,
    json_inputs: bool,
    full_ids: bool,
    id_map: &Map<String, Value>,
    registry: Option<&Registry>,
    outputs_used: &std::collections::HashMap<String, std::collections::HashSet<usize>>,
) -> Value {
    let name = node_meta.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let typ = node_meta.get("type").and_then(|v| v.as_str()).unwrap_or("");

    if matches!(mode, GraphMode::Summary) {
        let mut obj = Map::new();
        if include_ids {
            obj.insert("id".to_string(), Value::String(id.to_string()));
        }
        obj.insert("n".to_string(), Value::String(name.to_string()));
        obj.insert("t".to_string(), Value::String(typ.to_string()));
        return Value::Object(obj);
    }

    let node = nodes
        .iter()
        .find(|n| n.get("id").and_then(|v| v.as_str()) == Some(id));
    let mut base = Map::new();
    if include_ids {
        base.insert("id".to_string(), Value::String(id.to_string()));
    }
    base.insert("n".to_string(), Value::String(name.to_string()));
    base.insert("t".to_string(), Value::String(typ.to_string()));
    let mut out = Value::Object(base);

    if let Some(n) = node {
        if include_pos {
            if let Some(obj) = out.as_object_mut() {
                obj.insert(
                    "p".to_string(),
                    json!([
                        node.and_then(|n| n.get("x").and_then(|v| v.as_f64()))
                            .unwrap_or(0.0),
                        node.and_then(|n| n.get("y").and_then(|v| v.as_f64()))
                            .unwrap_or(0.0)
                    ]),
                );
            }
        }
        if let Some(attri) = n.get("attri") {
            if !matches!(mode, GraphMode::Compact) {
                out["a"] = attri.clone();
            }
        }
        let reg_node = registry.and_then(|r| r.nodes.get(typ));
        if let Some(inputs) = n.get("inputs").and_then(|v| v.as_array()) {
            let mut ins = Vec::new();
            for (i, input) in inputs.iter().enumerate() {
                let mut entry = Map::new();
                entry.insert("s".to_string(), Value::Number((i as i64).into()));
                if let Some(rn) = reg_node {
                    if let Some(rin) = rn.inputs.get(i) {
                        if let Some(nm) = &rin.name {
                            entry.insert("n".to_string(), Value::String(nm.clone()));
                        }
                        if let Some(tp) = &rin.ty {
                            if tp != "unknown" && tp != "output" {
                                entry.insert("t".to_string(), Value::String(tp.clone()));
                            }
                        }
                    }
                }
                let (val, anim_meta) = extract_input_value_with_anim(input, mode);
                if let Some(v) = val.clone() {
                    if v != Value::Number((-4).into()) {
                        entry.insert("v".to_string(), v);
                    }
                }
                if let Some(meta) = anim_meta {
                    for (k, v) in meta {
                        entry.insert(k, v);
                    }
                }
                if let Some(conn) = extract_connection(input, full_ids, id_map) {
                    entry.insert("c".to_string(), conn);
                }
                if let Some(attri) = input.get("attri") {
                    entry.insert("a".to_string(), attri.clone());
                }
                if json_inputs {
                    entry.insert("raw".to_string(), input.clone());
                }
                let include = match mode {
                    GraphMode::Summary => false,
                    GraphMode::Compact => entry.contains_key("c") || entry.contains_key("a"),
                    GraphMode::Full => true,
                };
                if include {
                    ins.push(Value::Object(entry));
                }
            }
            if !ins.is_empty() {
                out["i"] = Value::Array(ins);
            }
        }
        if let Some(outputs) = n.get("outputs").and_then(|v| v.as_array()) {
            let mut outs = Vec::new();
            let used = outputs_used.get(id);
            for i in 0..outputs.len() {
                if matches!(mode, GraphMode::Compact) {
                    if let Some(set) = used {
                        if !set.contains(&i) {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                let mut entry = Map::new();
                if i != 0 {
                    entry.insert("s".to_string(), Value::Number((i as i64).into()));
                }
                if let Some(rn) = reg_node {
                    if let Some(rout) = rn.outputs.get(i) {
                        if let Some(nm) = &rout.name {
                            entry.insert("n".to_string(), Value::String(nm.clone()));
                        }
                        if let Some(tp) = &rout.ty {
                            if tp != "unknown" && tp != "output" {
                                entry.insert("t".to_string(), Value::String(tp.clone()));
                            }
                        }
                    }
                }
                let has_detail = entry.len() > 1;
                if matches!(mode, GraphMode::Compact) && !has_detail {
                } else {
                    outs.push(Value::Object(entry));
                }
            }
            if !outs.is_empty() {
                out["o"] = Value::Array(outs);
            }
        }
    }

    out
}

fn extract_input_value(input: &Value) -> Option<Value> {
    let r = input.get("r")?;
    if let Some(obj) = r.as_object() {
        if let Some(d) = obj.get("d") {
            return Some(d.clone());
        }
    }
    if r.is_array() {
        return Some(r.clone());
    }
    None
}

fn extract_input_value_with_anim(
    input: &Value,
    mode: GraphMode,
) -> (Option<Value>, Option<Map<String, Value>>) {
    let mut anim = false;
    let mut key_count = None;
    let mut raw_anim = None;

    if let Some(r) = input.get("r") {
        if let Some(obj) = r.as_object() {
            if obj.get("d").is_none() {
                anim = true;
            }
            if let Some(d) = obj.get("d") {
                key_count = Some(1);
                if matches!(mode, GraphMode::Full) {
                    raw_anim = Some(r.clone());
                }
                return (
                    Some(d.clone()),
                    anim_meta_map(anim, key_count, raw_anim, mode),
                );
            }
        }
        if r.is_array() {
            anim = true;
            key_count = r.as_array().map(|a| a.len());
            if matches!(mode, GraphMode::Full) {
                raw_anim = Some(r.clone());
            }
        }
    }

    if input.get("anim").and_then(|v| v.as_bool()).unwrap_or(false) {
        anim = true;
    }

    let val = extract_input_value(input);
    (val, anim_meta_map(anim, key_count, raw_anim, mode))
}

fn anim_meta_map(
    anim: bool,
    key_count: Option<usize>,
    raw_anim: Option<Value>,
    mode: GraphMode,
) -> Option<Map<String, Value>> {
    if !anim {
        return None;
    }
    let mut meta = Map::new();
    meta.insert("an".to_string(), Value::Bool(true));
    if let Some(kc) = key_count {
        meta.insert("k".to_string(), Value::Number((kc as i64).into()));
    }
    if matches!(mode, GraphMode::Full) {
        if let Some(raw) = raw_anim {
            meta.insert("ad".to_string(), raw);
        }
    }
    Some(meta)
}

fn extract_connection(input: &Value, full_ids: bool, id_map: &Map<String, Value>) -> Option<Value> {
    let from = input.get("from_node")?.as_str()?;
    let from_index = input
        .get("from_index")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))?;
    let from_tag = input
        .get("from_tag")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)));
    let from_id = if full_ids {
        from.to_string()
    } else {
        short_for_id(id_map, from).unwrap_or_else(|| from.to_string())
    };
    let mut map = Map::new();
    map.insert("f".to_string(), Value::String(from_id));
    map.insert("fo".to_string(), Value::Number(from_index.into()));
    if let Some(tag) = from_tag {
        map.insert("tg".to_string(), Value::Number(tag.into()));
    }
    Some(Value::Object(map))
}
