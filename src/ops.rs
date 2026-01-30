use anyhow::{Result, anyhow};
use serde_json::{Map, Value, json};

use crate::ids::short_id;
use crate::registry::Registry;

pub(crate) fn resolve_input_slot(
    node: &Value,
    input_slot: Option<usize>,
    input_name: Option<&str>,
    registry: Option<&Registry>,
) -> Result<usize> {
    if let Some(s) = input_slot {
        return Ok(s);
    }
    let name = input_name.ok_or_else(|| anyhow!("--input or --input-name required"))?;
    let reg_node = registry
        .and_then(|r| {
            r.nodes
                .get(node.get("type").and_then(|v| v.as_str()).unwrap_or(""))
        })
        .ok_or_else(|| anyhow!("registry missing node type"))?;
    for (i, inp) in reg_node.inputs.iter().enumerate() {
        if let Some(nm) = &inp.name {
            if nm == name {
                return Ok(i);
            }
        }
    }
    Err(anyhow!("input name not found: {}", name))
}

pub(crate) fn resolve_output_slot(
    node: &Value,
    output_slot: Option<usize>,
    output_name: Option<&str>,
    registry: Option<&Registry>,
) -> Result<usize> {
    if let Some(slot) = output_slot {
        return Ok(slot);
    }
    let name = output_name.ok_or_else(|| anyhow!("output slot or name required"))?;
    let node_type = node.get("type").and_then(|v| v.as_str()).unwrap_or("");
    if let Some(reg) = registry {
        if let Some(reg_node) = reg.nodes.get(node_type) {
            for (idx, port) in reg_node.outputs.iter().enumerate() {
                if let Some(port_name) = &port.name {
                    if port_name == name {
                        return Ok(idx);
                    }
                }
            }
        }
    }
    Err(anyhow!("output name not found: {}", name))
}

pub(crate) fn resolve_node_id(node_arg: &str, nodes: &[Value]) -> Option<String> {
    if nodes
        .iter()
        .any(|n| n.get("id").and_then(|v| v.as_str()) == Some(node_arg))
    {
        return Some(node_arg.to_string());
    }
    let mut id_list = Vec::new();
    for n in nodes.iter() {
        if let Some(id) = n.get("id").and_then(|v| v.as_str()) {
            id_list.push(id.to_string());
        }
    }
    for (i, id) in id_list.iter().enumerate() {
        if short_id(i) == node_arg {
            return Some(id.clone());
        }
    }
    None
}

pub fn set_input_value_in_pxc(
    pxc: &mut crate::pxc::PxcFile,
    node_arg: &str,
    input_slot: Option<usize>,
    input_name: Option<&str>,
    value: Value,
    registry: Option<&Registry>,
) -> Result<()> {
    let nodes = pxc
        .json
        .get_mut("nodes")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("no nodes array found"))?;

    let node_id = resolve_node_id(node_arg, nodes)
        .ok_or_else(|| anyhow!("node id not found: {}", node_arg))?;
    let node = nodes
        .iter_mut()
        .find(|n| n.get("id").and_then(|v| v.as_str()) == Some(node_id.as_str()))
        .ok_or_else(|| anyhow!("node not found after resolve: {}", node_id))?;

    let slot = resolve_input_slot(node, input_slot, input_name, registry)?;

    let node_type = node
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let inputs = node
        .get_mut("inputs")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("node has no inputs array"))?;
    while inputs.len() <= slot {
        inputs.push(Value::Object(Map::new()));
    }
    let input = inputs
        .get_mut(slot)
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| anyhow!("input slot is not an object"))?;

    input.remove("from_node");
    input.remove("from_index");
    input.remove("from_tag");
    input.remove("anim");
    let mut final_value = value;
    if let Some(reg) = registry {
        if let Some(reg_node) = reg.nodes.get(&node_type) {
            if let Some(port) = reg_node.inputs.get(slot) {
                if let Some(ty) = &port.ty {
                    let ty_lower = ty.to_ascii_lowercase();
                    if ty_lower.contains("gradient") {
                        if !final_value.is_string() {
                            let s = serde_json::to_string(&final_value)?;
                            final_value = Value::String(s);
                        }
                    }
                }
            }
        }
    }
    input.insert("r".to_string(), json!({ "d": final_value }));
    Ok(())
}

pub fn get_input_value_in_pxc(
    pxc: &crate::pxc::PxcFile,
    node_arg: &str,
    input_slot: Option<usize>,
    input_name: Option<&str>,
    registry: Option<&Registry>,
) -> Result<Value> {
    let nodes = pxc
        .json
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("no nodes array found"))?;

    let node_id = resolve_node_id(node_arg, nodes)
        .ok_or_else(|| anyhow!("node id not found: {}", node_arg))?;
    let node = nodes
        .iter()
        .find(|n| n.get("id").and_then(|v| v.as_str()) == Some(node_id.as_str()))
        .ok_or_else(|| anyhow!("node not found after resolve: {}", node_id))?;

    let slot = resolve_input_slot(node, input_slot, input_name, registry)?;
    let inputs = node
        .get("inputs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("node has no inputs array"))?;
    let input = inputs
        .get(slot)
        .ok_or_else(|| anyhow!("input slot out of range"))?;
    if let Some(r) = input.get("r") {
        if let Some(obj) = r.as_object() {
            if let Some(d) = obj.get("d") {
                return Ok(d.clone());
            }
        }
        return Ok(r.clone());
    }
    Ok(Value::Null)
}

pub(crate) fn set_json_pointer(root: &mut Value, pointer: &str, value: Value) -> Result<()> {
    if pointer.is_empty() || pointer == "/" {
        *root = value;
        return Ok(());
    }
    let tokens: Vec<String> = pointer
        .split('/')
        .skip(1)
        .map(|t| t.replace("~1", "/").replace("~0", "~"))
        .collect();

    let mut cur = root;
    for i in 0..tokens.len() {
        let key = tokens[i].as_str();
        let is_last = i == tokens.len() - 1;
        if let Value::Object(map) = cur {
            if is_last {
                map.insert(key.to_string(), value);
                return Ok(());
            }
            if !map.contains_key(key) {
                map.insert(key.to_string(), Value::Object(Map::new()));
            }
            cur = map.get_mut(key).unwrap();
        } else if let Value::Array(arr) = cur {
            let idx: usize = key
                .parse()
                .map_err(|_| anyhow!("invalid array index in pointer: {}", key))?;
            if idx >= arr.len() {
                arr.resize(idx + 1, Value::Null);
            }
            if is_last {
                arr[idx] = value;
                return Ok(());
            }
            if arr[idx].is_null() {
                arr[idx] = Value::Object(Map::new());
            }
            cur = &mut arr[idx];
        } else {
            return Err(anyhow!("pointer does not resolve to an object/array"));
        }
    }
    Ok(())
}

pub(crate) fn remove_json_pointer(root: &mut Value, pointer: &str) -> Result<()> {
    if pointer.is_empty() || pointer == "/" {
        *root = Value::Null;
        return Ok(());
    }
    let tokens: Vec<String> = pointer
        .split('/')
        .skip(1)
        .map(|t| t.replace("~1", "/").replace("~0", "~"))
        .collect();

    let mut cur = root;
    for i in 0..tokens.len() {
        let key = tokens[i].as_str();
        let is_last = i == tokens.len() - 1;
        if let Value::Object(map) = cur {
            if is_last {
                map.remove(key);
                return Ok(());
            }
            cur = map
                .get_mut(key)
                .ok_or_else(|| anyhow!("pointer not found"))?;
        } else if let Value::Array(arr) = cur {
            let idx: usize = key
                .parse()
                .map_err(|_| anyhow!("invalid array index in pointer: {}", key))?;
            if idx >= arr.len() {
                return Err(anyhow!("pointer not found"));
            }
            if is_last {
                arr[idx] = Value::Null;
                return Ok(());
            }
            cur = &mut arr[idx];
        } else {
            return Err(anyhow!("pointer does not resolve to an object/array"));
        }
    }
    Ok(())
}
