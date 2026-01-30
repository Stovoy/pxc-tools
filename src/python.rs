// PyO3's macro-generated shims trigger Rust 2024 unsafe_op_in_unsafe_fn warnings.
// Scope the allow to this bindings module only (not the whole crate).
#![allow(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde_json::{Map, Value, json};

use crate::color::{
    color_from_rgba, color_from_value, default_gradient_value, gradient_value_from_keys,
};
use crate::graph::{GraphMode, graph_json_from_pxc};
use crate::ops::{
    get_input_value_in_pxc, remove_json_pointer, resolve_input_slot, resolve_node_id,
    resolve_output_slot, set_input_value_in_pxc, set_json_pointer,
};
use crate::pxc::{PxcFile, read_pxc, write_pxc};
use crate::registry::{RegistryPort, embedded_registry_inner};

static NODE_COUNTER: AtomicU64 = AtomicU64::new(1);

fn py_err<E: std::fmt::Display>(err: E) -> PyErr {
    PyRuntimeError::new_err(err.to_string())
}

fn py_any_to_value(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Value> {
    let json_mod = py.import_bound("json")?;
    let dumped = json_mod.call_method1("dumps", (value,))?;
    let s: String = dumped.extract()?;
    serde_json::from_str(&s).map_err(py_err)
}

fn display_name_from_type(node_type: &str) -> String {
    let base = node_type.strip_prefix("Node_").unwrap_or(node_type);
    base.replace('_', " ")
}

fn default_value_for_port(port: &RegistryPort) -> Value {
    let ty = port.ty.as_deref().unwrap_or("");
    let ty_lower = ty.to_ascii_lowercase();
    let name_lower = port.name.as_deref().unwrap_or("").to_ascii_lowercase();
    if ty_lower.contains("gradient") {
        return default_gradient_value();
    }
    if ty_lower.contains("color") || name_lower.contains("color") || name_lower.contains("colour") {
        return Value::Number(0xFFFF_FFFFu32.into());
    }
    if ty_lower.contains("toggle") || ty_lower.contains("bool") || name_lower.contains("enable") {
        return Value::Bool(false);
    }
    if ty_lower.contains("string") || ty_lower.contains("text") || ty_lower.contains("path") {
        return Value::String(String::new());
    }
    if ty_lower.contains("vector2") || ty_lower.contains("vec2") {
        return Value::Array(vec![Value::Number(0.into()), Value::Number(0.into())]);
    }
    if ty_lower.contains("vector3") || ty_lower.contains("vec3") {
        return Value::Array(vec![
            Value::Number(0.into()),
            Value::Number(0.into()),
            Value::Number(0.into()),
        ]);
    }
    if ty_lower.contains("vector4") || ty_lower.contains("vec4") {
        return Value::Array(vec![
            Value::Number(0.into()),
            Value::Number(0.into()),
            Value::Number(0.into()),
            Value::Number(0.into()),
        ]);
    }
    if ty_lower.contains("array") {
        return Value::Array(vec![]);
    }
    if ty_lower.contains("number")
        || ty_lower.contains("float")
        || ty_lower.contains("integer")
        || ty_lower.contains("slider")
    {
        return Value::Number(0.into());
    }
    Value::Null
}

#[pyclass]
struct Project {
    inner: PxcFile,
    path: Option<PathBuf>,
}

#[pymethods]
impl Project {
    #[staticmethod]
    fn load(path: &str) -> PyResult<Self> {
        let pxc = read_pxc(Path::new(path)).map_err(py_err)?;
        Ok(Project {
            inner: pxc,
            path: Some(PathBuf::from(path)),
        })
    }

    fn save(&mut self, path: Option<&str>) -> PyResult<()> {
        let target = if let Some(p) = path {
            PathBuf::from(p)
        } else if let Some(p) = &self.path {
            p.clone()
        } else {
            return Err(PyRuntimeError::new_err("no path provided"));
        };
        write_pxc(&target, &self.inner, true).map_err(py_err)?;
        Ok(())
    }

    fn dump(&self, pretty: Option<bool>) -> PyResult<String> {
        let s = if pretty.unwrap_or(false) {
            serde_json::to_string_pretty(&self.inner.json)
        } else {
            serde_json::to_string(&self.inner.json)
        }
        .map_err(py_err)?;
        Ok(s)
    }

    fn graph_json(
        &self,
        pretty: Option<bool>,
        include_id_map: Option<bool>,
        include_ids: Option<bool>,
        include_pos: Option<bool>,
        include_edges: Option<bool>,
        full_ids: Option<bool>,
        mode: Option<&str>,
    ) -> PyResult<String> {
        let mode = match mode.unwrap_or("compact") {
            "summary" => GraphMode::Summary,
            "full" => GraphMode::Full,
            _ => GraphMode::Compact,
        };
        let val = graph_json_from_pxc(
            &self.inner,
            mode,
            include_id_map.unwrap_or(false),
            include_ids.unwrap_or(false),
            include_pos.unwrap_or(false),
            false,
            full_ids.unwrap_or(false),
            include_edges.unwrap_or(false),
            None,
        )
        .map_err(py_err)?;
        let s = if pretty.unwrap_or(false) {
            serde_json::to_string_pretty(&val)
        } else {
            serde_json::to_string(&val)
        }
        .map_err(py_err)?;
        Ok(s)
    }

    fn get(&self, pointer: &str) -> PyResult<String> {
        let val = self
            .inner
            .json
            .pointer(pointer)
            .ok_or_else(|| PyRuntimeError::new_err("pointer not found"))?;
        serde_json::to_string(val).map_err(py_err)
    }

    #[pyo3(signature = (node, input=None, input_name=None))]
    fn get_input(
        &self,
        node: &str,
        input: Option<usize>,
        input_name: Option<&str>,
    ) -> PyResult<String> {
        let val = get_input_value_in_pxc(
            &self.inner,
            node,
            input,
            input_name,
            Some(&embedded_registry_inner()),
        )
        .map_err(py_err)?;
        serde_json::to_string(&val).map_err(py_err)
    }

    fn set(&mut self, pointer: &str, value_json: &str) -> PyResult<()> {
        let val: Value = serde_json::from_str(value_json).map_err(py_err)?;
        set_json_pointer(&mut self.inner.json, pointer, val).map_err(py_err)?;
        Ok(())
    }

    #[pyo3(signature = (pointer, value))]
    fn set_value(&mut self, py: Python<'_>, pointer: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let val = py_any_to_value(py, value)?;
        set_json_pointer(&mut self.inner.json, pointer, val).map_err(py_err)?;
        Ok(())
    }

    fn remove(&mut self, pointer: &str) -> PyResult<()> {
        remove_json_pointer(&mut self.inner.json, pointer).map_err(py_err)?;
        Ok(())
    }

    #[pyo3(signature = (node, value_json, input=None, input_name=None))]
    fn set_input(
        &mut self,
        node: &str,
        value_json: &str,
        input: Option<usize>,
        input_name: Option<&str>,
    ) -> PyResult<()> {
        let value: Value = serde_json::from_str(value_json).map_err(py_err)?;
        set_input_value_in_pxc(
            &mut self.inner,
            node,
            input,
            input_name,
            value,
            Some(&embedded_registry_inner()),
        )
        .map_err(py_err)?;
        Ok(())
    }

    #[pyo3(signature = (node, value, input=None, input_name=None))]
    fn set_input_value(
        &mut self,
        py: Python<'_>,
        node: &str,
        value: &Bound<'_, PyAny>,
        input: Option<usize>,
        input_name: Option<&str>,
    ) -> PyResult<()> {
        let value = py_any_to_value(py, value)?;
        set_input_value_in_pxc(
            &mut self.inner,
            node,
            input,
            input_name,
            value,
            Some(&embedded_registry_inner()),
        )
        .map_err(py_err)?;
        Ok(())
    }

    fn set_input_name(&mut self, node: &str, name: &str, value_json: &str) -> PyResult<()> {
        self.set_input(node, value_json, None, Some(name))
    }

    fn set_input_slot(&mut self, node: &str, slot: usize, value_json: &str) -> PyResult<()> {
        self.set_input(node, value_json, Some(slot), None)
    }

    fn batch_set_inputs(&mut self, ops_json: &str) -> PyResult<usize> {
        let ops: Value = serde_json::from_str(ops_json).map_err(py_err)?;
        let arr = ops
            .as_array()
            .ok_or_else(|| PyRuntimeError::new_err("ops_json must be a JSON array"))?;
        let registry = embedded_registry_inner();
        let mut changed = 0usize;
        for op in arr {
            let obj = op
                .as_object()
                .ok_or_else(|| PyRuntimeError::new_err("each op must be an object"))?;
            let node = obj
                .get("node")
                .and_then(|v| v.as_str())
                .ok_or_else(|| PyRuntimeError::new_err("op.node required"))?;
            let value = obj
                .get("value")
                .ok_or_else(|| PyRuntimeError::new_err("op.value required"))?
                .clone();
            let input_slot = obj
                .get("input")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            let input_name = obj.get("input_name").and_then(|v| v.as_str());
            set_input_value_in_pxc(
                &mut self.inner,
                node,
                input_slot,
                input_name,
                value,
                Some(&registry),
            )
            .map_err(py_err)?;
            changed += 1;
        }
        Ok(changed)
    }

    #[pyo3(signature = (node_type, x=None, y=None, name=None))]
    fn add_node(
        &mut self,
        node_type: &str,
        x: Option<i32>,
        y: Option<i32>,
        name: Option<&str>,
    ) -> PyResult<String> {
        let registry = embedded_registry_inner();
        let reg_node = registry
            .nodes
            .get(node_type)
            .ok_or_else(|| PyRuntimeError::new_err("unknown node type"))?;
        let mut inputs = Vec::with_capacity(reg_node.inputs.len());
        for port in &reg_node.inputs {
            let dv = default_value_for_port(port);
            inputs.push(json!({"m":1,"r":{"d": dv}}));
        }
        let mut outputs = Vec::with_capacity(reg_node.outputs.len());
        for _ in &reg_node.outputs {
            outputs.push(json!({}));
        }

        let base = display_name_from_type(node_type);
        let base_id = node_type
            .strip_prefix("Node_")
            .unwrap_or(node_type)
            .replace('_', "");
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let c = NODE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let id = format!("node{}_{}", ts, c);
        let iname = format!("{}{}", base_id, c);

        let node_name = name.map(|s| s.to_string()).unwrap_or_else(|| base.clone());
        let mut node = Map::new();
        node.insert("id".to_string(), Value::String(id.clone()));
        node.insert("type".to_string(), Value::String(node_type.to_string()));
        node.insert("name".to_string(), Value::String(node_name));
        node.insert("iname".to_string(), Value::String(iname));
        node.insert("x".to_string(), Value::Number((x.unwrap_or(0)).into()));
        node.insert("y".to_string(), Value::Number((y.unwrap_or(0)).into()));
        node.insert("version".to_string(), Value::Number(120000.into()));
        node.insert("renamed".to_string(), Value::Bool(name.is_some()));
        node.insert("data_length".to_string(), Value::Number(1.into()));
        node.insert(
            "input_fix_len".to_string(),
            Value::Number((reg_node.inputs.len() as u64).into()),
        );
        node.insert("inputs".to_string(), Value::Array(inputs));
        node.insert("outputs".to_string(), Value::Array(outputs));
        node.insert("attri".to_string(), Value::Object(Map::new()));
        node.insert("insp_col".to_string(), Value::Object(Map::new()));
        node.insert("inspectInputs".to_string(), Value::Array(vec![]));

        let nodes = self
            .inner
            .json
            .get_mut("nodes")
            .and_then(|v| v.as_array_mut())
            .ok_or_else(|| PyRuntimeError::new_err("nodes array missing"))?;
        nodes.push(Value::Object(node));
        Ok(id)
    }

    #[pyo3(signature = (from_node, to_node, from_output=None, to_input=None, to_input_name=None, from_output_name=None))]
    fn connect(
        &mut self,
        from_node: &str,
        to_node: &str,
        from_output: Option<usize>,
        to_input: Option<usize>,
        to_input_name: Option<&str>,
        from_output_name: Option<&str>,
    ) -> PyResult<()> {
        let registry = embedded_registry_inner();
        let nodes = self
            .inner
            .json
            .get_mut("nodes")
            .and_then(|v| v.as_array_mut())
            .ok_or_else(|| PyRuntimeError::new_err("nodes array missing"))?;

        let from_id = resolve_node_id(from_node, nodes)
            .ok_or_else(|| PyRuntimeError::new_err("from_node not found"))?;
        let to_id = resolve_node_id(to_node, nodes)
            .ok_or_else(|| PyRuntimeError::new_err("to_node not found"))?;

        let from_node_obj = nodes
            .iter()
            .find(|n| n.get("id").and_then(|v| v.as_str()) == Some(from_id.as_str()))
            .ok_or_else(|| PyRuntimeError::new_err("from_node not found after resolve"))?;
        let from_slot = resolve_output_slot(
            from_node_obj,
            from_output,
            from_output_name,
            Some(&registry),
        )
        .map_err(py_err)?;

        let to_node_obj = nodes
            .iter_mut()
            .find(|n| n.get("id").and_then(|v| v.as_str()) == Some(to_id.as_str()))
            .ok_or_else(|| PyRuntimeError::new_err("to_node not found after resolve"))?;
        let slot = resolve_input_slot(to_node_obj, to_input, to_input_name, Some(&registry))
            .map_err(py_err)?;

        let inputs = to_node_obj
            .get_mut("inputs")
            .and_then(|v| v.as_array_mut())
            .ok_or_else(|| PyRuntimeError::new_err("to_node inputs missing"))?;
        while inputs.len() <= slot {
            inputs.push(Value::Object(Map::new()));
        }
        let input = inputs
            .get_mut(slot)
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| PyRuntimeError::new_err("input slot is not an object"))?;

        input.insert("from_node".to_string(), Value::String(from_id));
        input.insert(
            "from_index".to_string(),
            Value::Number((from_slot as u64).into()),
        );
        input.remove("from_tag");
        Ok(())
    }

    #[pyo3(signature = (node))]
    fn set_preview_node(&mut self, node: &str) -> PyResult<()> {
        let nodes = self
            .inner
            .json
            .get("nodes")
            .and_then(|v| v.as_array())
            .ok_or_else(|| PyRuntimeError::new_err("nodes array missing"))?;
        let node_id = resolve_node_id(node, nodes)
            .ok_or_else(|| PyRuntimeError::new_err("node not found"))?;
        let root = self
            .inner
            .json
            .as_object_mut()
            .ok_or_else(|| PyRuntimeError::new_err("project root is not object"))?;
        root.insert("previewNode".to_string(), Value::String(node_id.clone()));
        root.insert("inspectingNode".to_string(), Value::String(node_id));
        Ok(())
    }

    #[pyo3(signature = (r, g, b, a=255))]
    fn add_color(&self, r: u8, g: u8, b: u8, a: u8) -> u32 {
        color_from_rgba(r, g, b, a)
    }

    #[pyo3(signature = (keys, interp=0))]
    fn add_gradient(&self, py: Python<'_>, keys: &Bound<'_, PyAny>, interp: i32) -> PyResult<String> {
        let value = py_any_to_value(py, keys)?;
        let arr = value.as_array().ok_or_else(|| {
            PyRuntimeError::new_err("keys must be an array of [time, color] pairs")
        })?;
        let mut parsed: Vec<(f64, u32)> = Vec::with_capacity(arr.len());
        for item in arr {
            let pair = item
                .as_array()
                .ok_or_else(|| PyRuntimeError::new_err("each key must be [time, color]"))?;
            if pair.len() != 2 {
                return Err(PyRuntimeError::new_err("each key must be [time, color]"));
            }
            let t = pair[0]
                .as_f64()
                .ok_or_else(|| PyRuntimeError::new_err("time must be a number"))?;
            let c = color_from_value(&pair[1])
                .ok_or_else(|| PyRuntimeError::new_err("color must be a 32-bit RGBA integer"))?;
            parsed.push((t, c));
        }
        let v = gradient_value_from_keys(&parsed, interp);
        match v {
            Value::String(s) => Ok(s),
            _ => Err(PyRuntimeError::new_err("gradient encode failed")),
        }
    }

    #[pyo3(signature = (node_type))]
    fn list_node_inputs_json(&self, node_type: &str) -> PyResult<String> {
        let registry = embedded_registry_inner();
        let node = registry
            .nodes
            .get(node_type)
            .ok_or_else(|| PyRuntimeError::new_err("unknown node type"))?;
        let val = serde_json::to_string(&node.inputs).map_err(py_err)?;
        Ok(val)
    }

    #[pyo3(signature = (node_type))]
    fn list_node_inputs(&self, py: Python<'_>, node_type: &str) -> PyResult<PyObject> {
        let json_str = self.list_node_inputs_json(node_type)?;
        let json_mod = py.import_bound("json")?;
        let loaded = json_mod.call_method1("loads", (json_str,))?;
        Ok(loaded.unbind())
    }

    #[pyo3(signature = (node_type))]
    fn list_node_outputs_json(&self, node_type: &str) -> PyResult<String> {
        let registry = embedded_registry_inner();
        let node = registry
            .nodes
            .get(node_type)
            .ok_or_else(|| PyRuntimeError::new_err("unknown node type"))?;
        let val = serde_json::to_string(&node.outputs).map_err(py_err)?;
        Ok(val)
    }

    #[pyo3(signature = (node_type))]
    fn list_node_outputs(&self, py: Python<'_>, node_type: &str) -> PyResult<PyObject> {
        let json_str = self.list_node_outputs_json(node_type)?;
        let json_mod = py.import_bound("json")?;
        let loaded = json_mod.call_method1("loads", (json_str,))?;
        Ok(loaded.unbind())
    }

    fn list_node_types_json(&self) -> PyResult<String> {
        let registry = embedded_registry_inner();
        let mut keys: Vec<String> = registry.nodes.keys().cloned().collect();
        keys.sort();
        serde_json::to_string(&keys).map_err(py_err)
    }

    fn list_node_types(&self, py: Python<'_>) -> PyResult<PyObject> {
        let json_str = self.list_node_types_json()?;
        let json_mod = py.import_bound("json")?;
        let loaded = json_mod.call_method1("loads", (json_str,))?;
        Ok(loaded.unbind())
    }

    fn hue_set_all(&mut self, hue_deg: f64) -> PyResult<usize> {
        let changed = crate::color::hue_set_pxc(&mut self.inner, hue_deg);
        Ok(changed)
    }

    fn color_to_rgba(&self, color: u32) -> (u8, u8, u8, u8) {
        let a = ((color >> 24) & 0xFF) as u8;
        let r = (color & 0xFF) as u8;
        let g = ((color >> 8) & 0xFF) as u8;
        let b = ((color >> 16) & 0xFF) as u8;
        (r, g, b, a)
    }

    fn rgba_to_color(&self, r: u8, g: u8, b: u8, a: u8) -> u32 {
        color_from_rgba(r, g, b, a)
    }
}

#[pymodule]
fn pxc(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Project>()?;
    Ok(())
}
