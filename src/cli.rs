use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use serde_json::{Map, Value};

use crate::graph::{GraphFormat, GraphMode, cmd_graph};
use crate::ops::{remove_json_pointer, set_input_value_in_pxc, set_json_pointer};
use crate::pxc::{
    PxcFile, decode_preview, read_pxc, rgba_bytes_to_image, write_pxc, zlib_decompress,
};
use crate::registry::cmd_registry_build;

#[derive(Parser)]
#[command(name = "pxc", version, about = "Pixel Composer .pxc project file tool")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Info {
        file: PathBuf,
    },
    Dump {
        file: PathBuf,
        #[arg(long)]
        pretty: bool,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Get {
        file: PathBuf,
        pointer: String,
    },
    Set {
        file: PathBuf,
        pointer: String,
        json: String,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        in_place: bool,
    },
    Rm {
        file: PathBuf,
        pointer: String,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        in_place: bool,
    },
    ListNodes {
        file: PathBuf,
    },
    Graph {
        file: PathBuf,
        #[arg(long, value_enum, default_value_t = GraphFormat::Json)]
        format: GraphFormat,
        #[arg(long, value_enum, default_value_t = GraphMode::Compact)]
        mode: GraphMode,
        #[arg(long)]
        pretty: bool,
        #[arg(long)]
        id_map: bool,
        #[arg(long)]
        include_ids: bool,
        #[arg(long)]
        pos: bool,
        #[arg(long)]
        json_inputs: bool,
        #[arg(long)]
        full_ids: bool,
        #[arg(long)]
        edges: bool,
        #[arg(long)]
        registry: Option<PathBuf>,
    },
    RegistryBuild {
        #[arg(long)]
        scripts: PathBuf,
        #[arg(long)]
        locale: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
    },
    SetInput {
        file: PathBuf,
        #[arg(long)]
        node: String,
        #[arg(long)]
        input: Option<usize>,
        #[arg(long)]
        input_name: Option<String>,
        #[arg(long)]
        value: Option<String>,
        #[arg(long)]
        value_file: Option<PathBuf>,
        #[arg(long)]
        registry: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        in_place: bool,
    },
    Connect {
        file: PathBuf,
        #[arg(long)]
        from: String,
        #[arg(long)]
        from_index: usize,
        #[arg(long)]
        to: String,
        #[arg(long)]
        to_input: usize,
        #[arg(long)]
        tag: Option<i64>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        in_place: bool,
    },
    ExtractPreview {
        file: PathBuf,
        out: PathBuf,
    },
    ExtractThumbnail {
        file: PathBuf,
        out: PathBuf,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Info { file } => cmd_info(&file),
        Command::Dump { file, pretty, out } => cmd_dump(&file, pretty, out),
        Command::Get { file, pointer } => cmd_get(&file, &pointer),
        Command::Set {
            file,
            pointer,
            json,
            out,
            in_place,
        } => cmd_set(&file, &pointer, &json, out, in_place),
        Command::Rm {
            file,
            pointer,
            out,
            in_place,
        } => cmd_rm(&file, &pointer, out, in_place),
        Command::ListNodes { file } => cmd_list_nodes(&file),
        Command::Graph {
            file,
            format,
            mode,
            pretty,
            id_map,
            include_ids,
            pos,
            json_inputs,
            full_ids,
            edges,
            registry,
        } => cmd_graph(
            &file,
            format,
            mode,
            pretty,
            id_map,
            include_ids,
            pos,
            json_inputs,
            full_ids,
            edges,
            registry.as_deref(),
        ),
        Command::RegistryBuild {
            scripts,
            locale,
            out,
        } => cmd_registry_build(&scripts, locale.as_deref(), &out),
        Command::SetInput {
            file,
            node,
            input,
            input_name,
            value,
            value_file,
            registry,
            out,
            in_place,
        } => cmd_set_input(
            &file,
            &node,
            input,
            input_name.as_deref(),
            value.as_deref(),
            value_file.as_deref(),
            registry.as_deref(),
            out,
            in_place,
        ),
        Command::Connect {
            file,
            from,
            from_index,
            to,
            to_input,
            tag,
            out,
            in_place,
        } => cmd_connect(&file, &from, from_index, &to, to_input, tag, out, in_place),
        Command::ExtractPreview { file, out } => cmd_extract_preview(&file, &out),
        Command::ExtractThumbnail { file, out } => cmd_extract_thumbnail(&file, &out),
    }
}

fn cmd_info(path: &Path) -> Result<()> {
    let pxc = read_pxc(path)?;
    let version = pxc
        .json
        .get("version")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)));
    let versions = pxc
        .json
        .get("versions")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let nodes = pxc
        .json
        .get("nodes")
        .and_then(|v| v.as_array())
        .map(|v| v.len())
        .unwrap_or(0);
    let preview = pxc.json.get("preview");
    let preview_kind = match preview {
        Some(Value::String(s)) if !s.is_empty() => "string",
        Some(Value::Object(_)) => "object",
        _ => "none",
    };

    println!("file: {}", path.display());
    println!("version: {}", version.unwrap_or(-1));
    println!("versions: {}", versions);
    println!("nodes: {}", nodes);
    println!("preview: {}", preview_kind);
    println!(
        "thumbnail: {}",
        if pxc.header.thumbnail.is_some() {
            "yes"
        } else {
            "no"
        }
    );
    println!("header_size: {}", pxc.header.header_size);
    if let Some(meta) = &pxc.header.meta {
        println!("meta.save_version: {}", meta.save_version);
        println!("meta.version_string: {}", meta.version_string);
    }
    if let Ok(preview) = decode_preview(&pxc.json) {
        println!(
            "preview.size: {}x{} (format {})",
            preview.width, preview.height, preview.format
        );
    }

    Ok(())
}

fn cmd_dump(path: &Path, pretty: bool, out: Option<PathBuf>) -> Result<()> {
    let pxc = read_pxc(path)?;
    let s = if pretty {
        serde_json::to_string_pretty(&pxc.json)?
    } else {
        serde_json::to_string(&pxc.json)?
    };

    if let Some(out_path) = out {
        std::fs::write(out_path, s)?;
    } else {
        println!("{}", s);
    }
    Ok(())
}

fn cmd_get(path: &Path, pointer: &str) -> Result<()> {
    let pxc = read_pxc(path)?;
    let val = pxc
        .json
        .pointer(pointer)
        .ok_or_else(|| anyhow!("pointer not found"))?;
    println!("{}", serde_json::to_string_pretty(val)?);
    Ok(())
}

fn cmd_set(
    path: &Path,
    pointer: &str,
    json_str: &str,
    out: Option<PathBuf>,
    in_place: bool,
) -> Result<()> {
    let mut pxc = read_pxc(path)?;
    let val: Value = serde_json::from_str(json_str)
        .with_context(|| "value must be valid JSON (wrap strings in quotes)")?;
    set_json_pointer(&mut pxc.json, pointer, val)?;
    write_with_target(path, out, in_place, &pxc)
}

fn cmd_rm(path: &Path, pointer: &str, out: Option<PathBuf>, in_place: bool) -> Result<()> {
    let mut pxc = read_pxc(path)?;
    remove_json_pointer(&mut pxc.json, pointer)?;
    write_with_target(path, out, in_place, &pxc)
}

fn cmd_list_nodes(path: &Path) -> Result<()> {
    let pxc = read_pxc(path)?;
    let nodes = pxc
        .json
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("no nodes array found"))?;
    for node in nodes {
        let id = node.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let typ = node.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let x = node.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let y = node.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
        println!("{}\t{}\t{}\t({}, {})", id, typ, name, x, y);
    }
    Ok(())
}

fn cmd_set_input(
    path: &Path,
    node_arg: &str,
    input_slot: Option<usize>,
    input_name: Option<&str>,
    value_json: Option<&str>,
    value_file: Option<&Path>,
    registry_path: Option<&Path>,
    out: Option<PathBuf>,
    in_place: bool,
) -> Result<()> {
    let mut pxc = read_pxc(path)?;
    let registry = crate::registry::load_registry(registry_path)?;

    let value_str = if let Some(s) = value_json {
        s.to_string()
    } else if let Some(p) = value_file {
        std::fs::read_to_string(p)?
    } else {
        return Err(anyhow!("--value or --value-file required"));
    };
    let value: Value =
        serde_json::from_str(&value_str).map_err(|e| anyhow!("invalid JSON value: {}", e))?;

    set_input_value_in_pxc(
        &mut pxc,
        node_arg,
        input_slot,
        input_name,
        value,
        registry.as_ref(),
    )?;

    write_with_target(path, out, in_place, &pxc)
}

fn cmd_connect(
    path: &Path,
    from: &str,
    from_index: usize,
    to: &str,
    to_input: usize,
    tag: Option<i64>,
    out: Option<PathBuf>,
    in_place: bool,
) -> Result<()> {
    let mut pxc = read_pxc(path)?;
    let nodes = pxc
        .json
        .get_mut("nodes")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("no nodes array found"))?;

    let mut target_node = None;
    for node in nodes.iter_mut() {
        if node.get("id").and_then(|v| v.as_str()) == Some(to) {
            target_node = Some(node);
            break;
        }
    }

    let node = target_node.ok_or_else(|| anyhow!("target node not found"))?;
    let inputs = node
        .get_mut("inputs")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow!("node has no inputs"))?;

    if to_input >= inputs.len() {
        bail!("input index out of range");
    }

    let mut map = inputs[to_input]
        .as_object()
        .cloned()
        .unwrap_or_else(Map::new);
    map.insert("from_node".to_string(), Value::String(from.to_string()));
    map.insert("from_index".to_string(), Value::Number(from_index.into()));
    if let Some(tag_val) = tag {
        map.insert("from_tag".to_string(), Value::Number(tag_val.into()));
    } else {
        map.remove("from_tag");
    }
    inputs[to_input] = Value::Object(map);

    write_with_target(path, out, in_place, &pxc)
}

fn cmd_extract_preview(path: &Path, out: &Path) -> Result<()> {
    let pxc = read_pxc(path)?;
    let preview = decode_preview(&pxc.json)?;
    let img = rgba_bytes_to_image(&preview.raw, preview.width, preview.height)?;
    img.save(out)?;
    Ok(())
}

fn cmd_extract_thumbnail(path: &Path, out: &Path) -> Result<()> {
    let pxc = read_pxc(path)?;
    let thumb = pxc
        .header
        .thumbnail
        .ok_or_else(|| anyhow!("no thumbnail in file"))?;
    let raw = zlib_decompress(&thumb.compressed)?;
    let size = (raw.len() as f64 / 4.0).sqrt() as u32;
    if size * size * 4 != raw.len() as u32 {
        bail!("thumbnail size is not a square RGBA buffer");
    }
    let img = rgba_bytes_to_image(&raw, size, size)?;
    img.save(out)?;
    Ok(())
}

fn write_with_target(
    path: &Path,
    out: Option<PathBuf>,
    in_place: bool,
    pxc: &PxcFile,
) -> Result<()> {
    let target = match (out, in_place) {
        (Some(p), _) => p,
        (None, true) => path.to_path_buf(),
        (None, false) => bail!("use --out or --in-place for write operations"),
    };
    write_pxc(&target, pxc, true)?;
    Ok(())
}
