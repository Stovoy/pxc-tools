# pxc-tools

Rust CLI and Python bindings for working with Pixel Composer `.pxc` project files.

- Read and write `.pxc` files (PXCX container + compressed JSON payload)
- Inspect and modify JSON via JSON Pointer
- Export node graphs in JSON, Mermaid, or Graphviz DOT
- Modify node inputs programmatically (CLI or Python)
- Extract preview and thumbnail images
- Build a node registry from Pixel Composer source (optional)

## Requirements

- Rust toolchain (stable)
- Python 3.8+ for the Python module
- Pixel Composer `.pxc` files to work with

## Install

### Rust CLI

Build from source:

```sh
cargo build --release
```

The CLI binary will be at:

- `target/release/pxc-tools` (Linux/macOS)
- `target/release/pxc-tools.exe` (Windows)

You can also run without building a release binary:

```sh
cargo run -- <command> ...
```

### Python module (local install)

The Python module is built with PyO3 and installed into your **user site-packages**.
Import it as:

- `import pxc`

Quick install:

```sh
python scripts/install_python_module.py
```

This script:

- builds the native module with `cargo build --features python --lib`
- copies `pxc.pyd` (Windows) into your user site-packages

## CLI quickstart

```sh
# Summary
cargo run -- info project.pxc

# Dump JSON (pretty)
cargo run -- dump project.pxc --pretty

# Get/set/remove JSON by JSON Pointer
cargo run -- get project.pxc /version
cargo run -- set project.pxc /metadata/author '"Your Name"' --in-place
cargo run -- rm project.pxc /notes/0 --out ../edited.pxc

# List nodes
cargo run -- list-nodes project.pxc

# Graph export
cargo run -- graph project.pxc
cargo run -- graph project.pxc --pretty --id-map --include-ids
cargo run -- graph project.pxc --mode full --edges
cargo run -- graph project.pxc --format summary
cargo run -- graph project.pxc --format mermaid
cargo run -- graph project.pxc --format dot

# Build registry (input/output names + inferred types)
cargo run -- registry-build --scripts ../Pixel-Composer/scripts --locale ../Pixel-Composer/datafiles/data/Locale/en/nodes.json --out registry.json

# Connect node output -> node input
cargo run -- connect project.pxc --from <node_id> --from-index 0 --to <node_id> --to-input 3 --in-place

# Preview/thumbnail
cargo run -- extract-preview project.pxc preview.png
cargo run -- extract-thumbnail project.pxc thumb.png
```

## Python API

The Python module exposes a single class: `pxc.Project`.
All JSON values are encoded/decoded using JSON strings (unless you use the
`*_value` helpers that accept native Python values).

### Load and save

```py
from pxc import Project

project = Project.load("/path/to/file.pxc")
project.save()                  # overwrite original
project.save("/path/to/out.pxc")
```

### JSON accessors

```py
# JSON Pointer access
version_json = project.get("/version")        # returns a JSON string
project.set("/metadata/author", '"Ada"')      # value_json must be valid JSON

# Use Python values directly
project.set_value("/metadata/author", "Ada")
project.set_value("/metadata/flags", {"x": True, "y": 3})
```

### Node inputs

```py
# Get input value (JSON string)
value_json = project.get_input("node123", input=0)
value_json = project.get_input("node123", input_name="Strength")

# Set input value from JSON
project.set_input("node123", "0.25", input=0)

# Set input value from a Python object
project.set_input_value("node123", 0.25, input=0)

# Convenience helpers
project.set_input_name("node123", "Strength", "0.25")
project.set_input_slot("node123", 0, "0.25")
```

`node` can be a full node id or a short id (A, B, C, ...) based on the current
node order in the file.

### Batch input edits

```py
ops = [
    {"node": "node123", "input": 0, "value": 0.5},
    {"node": "node123", "input_name": "Strength", "value": 0.75},
]
project.batch_set_inputs(json.dumps(ops))
```

### Graph export

```py
graph_json = project.graph_json(
    pretty=True,
    include_id_map=True,
    include_ids=False,
    include_pos=True,
    include_edges=True,
    full_ids=False,
    mode="compact",   # "summary" | "compact" | "full"
)
```

### Create nodes and connections

```py
new_id = project.add_node("Node_Blend", x=100, y=200, name="Blend")
project.connect(
    from_node="nodeA",
    to_node=new_id,
    from_output=0,
    to_input=1,
)
project.set_preview_node(new_id)
```

### Colors and gradients

Pixel Composer stores colors as 32-bit integers in the format `0xAABBGGRR`.

```py
color = project.add_color(255, 128, 0, 255)  # r, g, b, a -> 0xAABBGGRR
r, g, b, a = project.color_to_rgba(color)
color2 = project.rgba_to_color(r, g, b, a)

# Gradient keys: [[time, color], ...]
encoded = project.add_gradient([
    [0.0, color],
    [1.0, project.add_color(0, 0, 0, 255)],
], interp=0)
```

### Registry helpers

```py
node_types = project.list_node_types()          # Python list
node_types_json = project.list_node_types_json()

inputs = project.list_node_inputs("Node_Blend")
outputs = project.list_node_outputs("Node_Blend")
```

### Full API reference

`Project` methods:

- `load(path: str) -> Project`
- `save(path: Optional[str] = None) -> None`
- `dump(pretty: Optional[bool] = None) -> str`
- `graph_json(pretty=None, include_id_map=None, include_ids=None, include_pos=None, include_edges=None, full_ids=None, mode=None) -> str`
- `get(pointer: str) -> str`
- `set(pointer: str, value_json: str) -> None`
- `set_value(pointer: str, value: Any) -> None`
- `remove(pointer: str) -> None`
- `get_input(node: str, input: Optional[int] = None, input_name: Optional[str] = None) -> str`
- `set_input(node: str, value_json: str, input: Optional[int] = None, input_name: Optional[str] = None) -> None`
- `set_input_value(node: str, value: Any, input: Optional[int] = None, input_name: Optional[str] = None) -> None`
- `set_input_name(node: str, name: str, value_json: str) -> None`
- `set_input_slot(node: str, slot: int, value_json: str) -> None`
- `batch_set_inputs(ops_json: str) -> int`
- `add_node(node_type: str, x: Optional[int] = None, y: Optional[int] = None, name: Optional[str] = None) -> str`
- `connect(from_node: str, to_node: str, from_output: Optional[int] = None, to_input: Optional[int] = None, to_input_name: Optional[str] = None, from_output_name: Optional[str] = None) -> None`
- `set_preview_node(node: str) -> None`
- `add_color(r: int, g: int, b: int, a: int = 255) -> int`
- `add_gradient(keys: Any, interp: int = 0) -> str`
- `list_node_inputs_json(node_type: str) -> str`
- `list_node_inputs(node_type: str) -> list`
- `list_node_outputs_json(node_type: str) -> str`
- `list_node_outputs(node_type: str) -> list`
- `list_node_types_json() -> str`
- `list_node_types() -> list`
- `hue_set_all(hue_deg: float) -> int`
- `color_to_rgba(color: int) -> (int, int, int, int)`
- `rgba_to_color(r: int, g: int, b: int, a: int) -> int`

## Project file format (reverse-engineered)

### Container

All Pixel Composer project files use a small header followed by a compressed JSON payload.

```
Offset  Size  Description
0x00    4     ASCII "PXCX"
0x04    4     u32 little-endian header_size (start of payload)
0x08    ...   Optional chunks (THMB, META, etc.)
header_size..  zlib-compressed JSON payload
```

Chunks appear between `0x08` and `header_size`:

- `THMB` (optional)
  - `u32` length
  - zlib-compressed raw RGBA bytes (no header)
  - The decoded buffer is square; size = sqrt(len/4)
- `META`
  - `u32` length
  - `u32` save_version
  - null-terminated version string (UTF-8)

The payload is zlib-compressed bytes produced by GameMaker’s `buffer_compress_string`:
- content is a **null-terminated JSON string**
- if decompression fails, treat the payload as a plain null-terminated JSON string

### Top-level JSON

Produced by `Project.serialize()` in `scripts/project_data/project_data.gml`.
Important keys (not exhaustive):

- `version` (number) - SAVE_VERSION
- `versions` (string) - VERSION_STRING
- `is_nightly` (bool)
- `freeze` (bool)
- `animator` (struct)
- `metadata` (struct)
- `global_node` (struct)
- `onion_skin` (struct)
- `previewNode` (node id string)
- `inspectingNode` (node id string)
- `previewGrid`, `graphGrid`, `graphConnection` (structs)
- `attributes` (struct)
- `data` (struct)
- `timelines` (struct)
- `notes` (array)
- `trackAnim`, `randomizer` (structs)
- `composer` (number)
- `load_layout` (bool)
- `layout` (struct, optional when `load_layout` is true)
- `graph_display_parameter` (struct)
- `favVal` (array of `[node_id, input_index]`)
- `cPanels` (custom panels)
- `nodes` (array of nodes)
- `preview` (stringified JSON from `surface_encode`)
- `addon` (struct)

### Preview image (`preview`)

`preview` is usually a JSON string, produced by `surface_encode()`:

```json
{
  "width": 128,
  "height": 128,
  "buffer": "<base64(zlib(raw bytes))>",
  "format": 6
}
```

- `buffer` is base64 of zlib-compressed raw surface bytes.
- `format` is a surface format id; `6` corresponds to `surface_rgba8unorm`.

### Nodes (`nodes[]`)

Node serialization is in `scripts/node_data/node_data.gml`.
Common fields:

- `id` (string) - node id
- `type` (string) - node class name (e.g. `Node_Blend`)
- `name` (string) - display name
- `iname` (string) - internal name
- `x`, `y` (numbers)
- `group` (node id or `noone`)
- `ictx` (inline context id)
- `render` (bool) - renderActive
- `previewable` (bool)
- `show_parameter` (bool)
- `insp_scr` (number) - inspector scroll
- `insp_col` (struct) - collapsed inspector state
- `visible` (bool)
- `is_instancer` (bool)
- `attri` (struct) - node attributes (stripped defaults)
- `input_fix_len`, `data_length` for dynamic inputs
- `inputs` (array of input junctions)
- `outputs` (array of output junctions)
- `inspectInputs` (array of special inputs)
- `outputMeta` (array, optional)
- `renamed` (bool)
- `instanceBase` (node id)

Node-specific fields are added by `doSerialize()` / `processSerialize()`.

### Junctions (inputs/outputs)

Serialized in `scripts/node_value/node_value.gml`.
For input junctions, possible fields include:

- `v`, `visible`, `visible_manual`
- `color`, `drawValue`
- `insp_tm` (timeline flag)
- `graph_h`, `graph_sh`, `graph_shs`
- `name`, `name_custom`
- `unit`, `on_end`, `loop_range`, `sep_axis`, `favorited`
- `shift_x`, `shift_y`, `shift_e`
- `m` (modified)
- `from_node`, `from_index`, `from_tag` (connection)
- `global_use`, `global_key` (expression)
- `anim`, `ign_array`
- `r` (animator data)
- `animators` (per-axis anims)
- `bypass`
- `linked`, `ranged`
- `attri` (junction attributes)

Outputs serialize only visual fields; connections are stored on inputs.

### Connections

Connections are stored on **input junctions**:

```json
{ "from_node": "<node_id>", "from_index": 0 }
```

Optional `from_tag` connects to special outputs:
- `VALUE_TAG.updateInTrigger`
- `VALUE_TAG.updateOutTrigger`
- `VALUE_TAG.matadata`

Bypass connections use `from_index >= 1000`.

### Notes on compression

- The JSON payload is always zlib-compressed when saved by Pixel Composer.
- The JSON string is null-terminated in the payload, matching `buffer_string`.

## Source references

Key logic in Pixel Composer source:

- `scripts/save_function/save_function.gml`
- `scripts/load_function/load_function.gml`
- `scripts/project_data/project_data.gml`
- `scripts/node_data/node_data.gml`
- `scripts/node_value/node_value.gml`
- `scripts/surface_functions/surface_functions.gml`
- `scripts/buffer_functions/buffer_functions.gml`
