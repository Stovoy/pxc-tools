import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = "git@github.com:Ttanasart-pt/Pixel-Composer.git"
SHA = "efee9dfbf21feefb590751021671b5ca1b551d67"
OUT_FILE = Path("pxc-tools/src/registry_embedded.json")


def run(cmd, cwd=None):
    subprocess.run(cmd, cwd=cwd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.STDOUT)


def parse_locale(locale_path: Path):
    if not locale_path.exists():
        return {}
    data = json.loads(locale_path.read_text(encoding="utf-8"))
    out = {}
    for node_name, node_val in data.items():
        ins = node_val.get("inputs") or []
        outs = node_val.get("outputs") or []
        out[node_name] = {"inputs": ins, "outputs": outs}
    return out


def parse_scripts(scripts_dir: Path):
    node_fn_re = re.compile(r"function\s+(Node_[A-Za-z0-9_]+)")
    new_input_re = re.compile(r"newInput[^,]*,\s*(new\s+)?([A-Za-z_][A-Za-z0-9_]*)")
    new_output_re = re.compile(r"newOutput[^,]*,\s*(new\s+)?([A-Za-z_][A-Za-z0-9_]*)")
    value_type_re = re.compile(r"VALUE_TYPE\.([A-Za-z0-9_]+)")
    int_re = re.compile(r"(\d+)")

    nodes = {}
    for gml in scripts_dir.rglob("*.gml"):
        text = gml.read_text(encoding="utf-8", errors="ignore")
        m = node_fn_re.search(text)
        if not m:
            continue
        node_name = m.group(1)
        inputs = {}
        outputs = {}
        for cap in new_input_re.finditer(text):
            whole = cap.group(0)
            ty = None
            mty = value_type_re.search(whole)
            if mty:
                ty = mty.group(1)
            else:
                ty = cap.group(2)
            mi = int_re.search(whole)
            if mi:
                idx = int(mi.group(1))
                inputs[idx] = {"name": None, "type": ty, "tooltip": None}
        for cap in new_output_re.finditer(text):
            whole = cap.group(0)
            ty = None
            mty = value_type_re.search(whole)
            if mty:
                ty = mty.group(1)
            else:
                ty = cap.group(2)
            mi = int_re.search(whole)
            if mi:
                idx = int(mi.group(1))
                outputs[idx] = {"name": None, "type": ty, "tooltip": None}
        nodes[node_name] = {"inputs": inputs, "outputs": outputs}
    return nodes


def merge(nodes, locale):
    for node_name, node_val in locale.items():
        if node_name not in nodes:
            nodes[node_name] = {"inputs": {}, "outputs": {}}
        for i, entry in enumerate(node_val.get("inputs") or []):
            if i not in nodes[node_name]["inputs"]:
                nodes[node_name]["inputs"][i] = {"name": None, "type": None, "tooltip": None}
            for k in ("name", "type", "tooltip"):
                if entry.get(k) is not None:
                    nodes[node_name]["inputs"][i][k] = entry.get(k)
        for i, entry in enumerate(node_val.get("outputs") or []):
            if i not in nodes[node_name]["outputs"]:
                nodes[node_name]["outputs"][i] = {"name": None, "type": None, "tooltip": None}
            for k in ("name", "type", "tooltip"):
                if entry.get(k) is not None:
                    nodes[node_name]["outputs"][i][k] = entry.get(k)


def rust_str(s: str) -> str:
    return s.replace("\\", "\\\\").replace("\"", "\\\"").replace("\r", "\\r").replace("\n", "\\n")


def opt_str(s):
    if s is None:
        return "None"
    if not isinstance(s, str):
        return "None"
    return f"Some(\"{rust_str(s)}\".to_string())"


def write_registry(nodes, out_path: Path):
    out = {}
    for node_name in sorted(nodes.keys()):
        node = nodes[node_name]
        ins = []
        outs = []
        in_keys = sorted(node["inputs"].keys()) if node["inputs"] else []
        max_in = in_keys[-1] if in_keys else -1
        for i in range(max_in + 1):
            p = node["inputs"].get(i, {"name": None, "type": None, "tooltip": None})
            ins.append({"name": p.get("name"), "type": p.get("type"), "tooltip": p.get("tooltip")})
        out_keys = sorted(node["outputs"].keys()) if node["outputs"] else []
        max_out = out_keys[-1] if out_keys else -1
        for i in range(max_out + 1):
            p = node["outputs"].get(i, {"name": None, "type": None, "tooltip": None})
            outs.append({"name": p.get("name"), "type": p.get("type"), "tooltip": p.get("tooltip")})
        out[node_name] = {"inputs": ins, "outputs": outs}
    out_path.write_text(json.dumps(out, ensure_ascii=False), encoding="utf-8")


def main():
    tmp = Path(tempfile.mkdtemp(prefix="pxc-registry-"))
    try:
        run(["git", "clone", REPO, str(tmp)])
        run(["git", "checkout", SHA], cwd=tmp)
        scripts_dir = tmp / "scripts"
        locale_path = tmp / "datafiles" / "data" / "Locale" / "en" / "nodes.json"

        nodes = parse_scripts(scripts_dir)
        locale = parse_locale(locale_path)
        merge(nodes, locale)
        write_registry(nodes, OUT_FILE)
        print(f"Wrote registry to {OUT_FILE}")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    main()
