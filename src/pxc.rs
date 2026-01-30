use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use image::{DynamicImage, ImageBuffer, Rgba};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Thumbnail {
    pub compressed: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct Meta {
    pub save_version: u32,
    pub version_string: String,
}

#[derive(Clone, Debug)]
pub struct Header {
    pub thumbnail: Option<Thumbnail>,
    pub meta: Option<Meta>,
    pub header_size: u32,
}

#[derive(Clone, Debug)]
pub struct PxcFile {
    pub header: Header,
    pub json: Value,
}

pub fn read_pxc(path: &Path) -> Result<PxcFile> {
    let data = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    parse_pxc(&data)
}

pub fn parse_pxc(data: &[u8]) -> Result<PxcFile> {
    if data.len() < 8 {
        bail!("file too small");
    }

    if &data[0..4] != b"PXCX" {
        let json = parse_payload(data)?;
        return Ok(PxcFile {
            header: Header {
                thumbnail: None,
                meta: None,
                header_size: 0,
            },
            json,
        });
    }

    let mut rdr = io::Cursor::new(data);
    let mut magic = [0u8; 4];
    rdr.read_exact(&mut magic)?;
    let header_size = rdr.read_u32::<LittleEndian>()?;

    if header_size < 8 {
        bail!("header_size too small");
    }
    if header_size as usize > data.len() {
        bail!("header_size beyond file length");
    }

    let mut thumbnail = None;
    let mut meta = None;

    let mut pos = rdr.position() as u32;
    while pos < header_size {
        let remaining = header_size
            .checked_sub(pos)
            .ok_or_else(|| anyhow!("header_size underflow"))?;
        if remaining < 8 {
            bail!("truncated chunk header");
        }
        let mut tag = [0u8; 4];
        rdr.read_exact(&mut tag)?;
        pos += 4;
        let tag_str = std::str::from_utf8(&tag).unwrap_or("????");

        let len = rdr.read_u32::<LittleEndian>()?;
        pos += 4;

        let remaining = header_size
            .checked_sub(pos)
            .ok_or_else(|| anyhow!("header_size underflow"))?;
        if len > remaining {
            bail!("chunk length exceeds header size");
        }

        let mut buf = vec![0u8; len as usize];
        rdr.read_exact(&mut buf)?;
        pos += len;

        match tag_str {
            "THMB" => {
                thumbnail = Some(Thumbnail { compressed: buf });
            }
            "META" => {
                if len < 4 {
                    bail!("META chunk too small");
                }
                let mut meta_rdr = io::Cursor::new(buf);
                let save_version = meta_rdr.read_u32::<LittleEndian>()?;
                let mut str_buf = Vec::new();
                meta_rdr.read_to_end(&mut str_buf)?;
                let version_string = trim_cstr(&str_buf);
                meta = Some(Meta {
                    save_version,
                    version_string,
                });
            }
            _ => {}
        }
    }

    let payload = &data[header_size as usize..];
    let json = parse_payload(payload)?;

    Ok(PxcFile {
        header: Header {
            thumbnail,
            meta,
            header_size,
        },
        json,
    })
}

fn parse_payload(payload: &[u8]) -> Result<Value> {
    let decoded = match zlib_decompress(payload) {
        Ok(v) => v,
        Err(_) => payload.to_vec(),
    };
    let s = trim_cstr(&decoded);
    serde_json::from_str(&s).with_context(|| "failed to parse JSON payload")
}

fn trim_cstr(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).to_string()
}

pub(crate) fn zlib_decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    Ok(out)
}

pub(crate) fn zlib_compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

pub fn write_pxc(path: &Path, pxc: &PxcFile, minify: bool) -> Result<()> {
    let json_str = if minify {
        serde_json::to_string(&pxc.json)?
    } else {
        serde_json::to_string_pretty(&pxc.json)?
    };
    let payload = {
        let mut s = json_str.into_bytes();
        s.push(0);
        zlib_compress(&s)?
    };

    let mut buf = Vec::new();
    buf.extend_from_slice(b"PXCX");
    buf.write_u32::<LittleEndian>(0)?;

    if let Some(thumb) = &pxc.header.thumbnail {
        buf.extend_from_slice(b"THMB");
        buf.write_u32::<LittleEndian>(thumb.compressed.len() as u32)?;
        buf.extend_from_slice(&thumb.compressed);
    }

    let meta = pxc
        .header
        .meta
        .clone()
        .or_else(|| derive_meta_from_json(&pxc.json));
    if let Some(meta) = meta {
        let mut meta_buf = Vec::new();
        meta_buf.write_u32::<LittleEndian>(meta.save_version)?;
        meta_buf.extend_from_slice(meta.version_string.as_bytes());
        meta_buf.push(0);

        buf.extend_from_slice(b"META");
        buf.write_u32::<LittleEndian>(meta_buf.len() as u32)?;
        buf.extend_from_slice(&meta_buf);
    }

    let header_size = buf.len() as u32;
    let mut cursor = io::Cursor::new(&mut buf[4..8]);
    cursor.write_u32::<LittleEndian>(header_size)?;

    buf.extend_from_slice(&payload);
    fs::write(path, buf)?;
    Ok(())
}

fn derive_meta_from_json(json: &Value) -> Option<Meta> {
    let save_version = json.get("version")?.as_i64()? as u32;
    let version_string = json
        .get("versions")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Some(Meta {
        save_version,
        version_string,
    })
}

pub(crate) struct PreviewData {
    pub width: u32,
    pub height: u32,
    pub raw: Vec<u8>,
    pub format: i64,
}

pub(crate) fn decode_preview(json: &Value) -> Result<PreviewData> {
    let preview_val = json
        .get("preview")
        .ok_or_else(|| anyhow!("no preview field"))?;

    let preview_obj = match preview_val {
        Value::String(s) if !s.is_empty() => serde_json::from_str::<Value>(s)?,
        Value::Object(_) => preview_val.clone(),
        _ => bail!("preview is empty"),
    };

    let obj = preview_obj
        .as_object()
        .ok_or_else(|| anyhow!("preview is not an object"))?;

    let width = obj
        .get("width")
        .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
        .unwrap_or(0) as u32;
    let height = obj
        .get("height")
        .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
        .unwrap_or(0) as u32;
    let format = obj
        .get("format")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        .unwrap_or(6);
    let buffer = obj
        .get("buffer")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("preview buffer missing"))?;

    if width == 0 || height == 0 {
        bail!("preview has invalid dimensions");
    }
    if format != 6 {
        bail!(
            "preview format {} not supported (expected 6 == rgba8)",
            format
        );
    }

    let compressed = general_purpose::STANDARD
        .decode(buffer)
        .context("base64 decode failed")?;
    let raw = zlib_decompress(&compressed)?;
    if raw.len() != (width * height * 4) as usize {
        bail!("preview buffer size mismatch");
    }

    Ok(PreviewData {
        width,
        height,
        raw,
        format,
    })
}

pub(crate) fn rgba_bytes_to_image(raw: &[u8], width: u32, height: u32) -> Result<DynamicImage> {
    let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(width, height, raw.to_vec())
        .ok_or_else(|| anyhow!("failed to build image buffer"))?;
    Ok(DynamicImage::ImageRgba8(img))
}
