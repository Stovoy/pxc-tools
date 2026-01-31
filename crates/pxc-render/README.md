# pxc-render

Rust CLI and library for fully headless rendering of Pixel Composer `.pxc` projects.

This sub-crate is intended to **replace the Pixel Composer GUI runtime** for rendering and
validation. The goal is byte-for-byte equivalent output with no dependency on Pixel Composer
itself. This is driven directly by the open-source Pixel Composer code in `pixel-composer-src/`.

Status: scaffold only. The engine, node implementations, and validation logic are not yet
implemented.

## Scope

- Parse PXCX container and JSON payloads
- Build a typed node graph from `.pxc`
- Execute graph end-to-end with caching and deterministic evaluation
- Implement **all nodes** from `src/registry_embedded.json`
- Implement input animation, timelines, and per-input animators
- Reproduce Pixel Composer surface/image behavior and shader math
- Provide a CLI to render and validate without Pixel Composer

## CLI (planned)

```
pxc-render render <project.pxc> --out out.png [--frame N] [--preview-node ID] [--validate]
pxc-render validate <project.pxc>
```

## Sub-crate layout

- `src/project.rs`: PXC parsing, JSON model, and type decoding
- `src/runtime.rs`: core value types (colors, gradients, surfaces, arrays)
- `src/nodes/`: node catalog and per-node implementations
- `src/render.rs`: graph execution + rendering pipeline
- `src/validate.rs`: syntax + semantic validation
- `NODE_CHECKLIST.md`: full node implementation checklist

## Primary references

Pixel Composer sources in this repo:

- `pixel-composer-src/scripts/`
- `src/registry_embedded.json`

Implementation should follow GML behavior as closely as possible (including edge cases),
and remain deterministic across platforms.
