use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

use anyhow::{Context, anyhow, bail};
use crc32fast::Hasher as Crc32Hasher;
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{ProjectDocument, SourceDocument};

pub const DOCUMENT_ARCHIVE_FORMAT_VERSION: u32 = 4;
pub const SCENE_JSON_NAME: &str = "scene.json";
pub const RAW_GDS_DIR: &str = "gds";

const ZIP_LOCAL_HEADER_SIGNATURE: u32 = 0x0403_4b50;
const ZIP_CENTRAL_HEADER_SIGNATURE: u32 = 0x0201_4b50;
const ZIP_END_SIGNATURE: u32 = 0x0605_4b50;
const ZIP_STORE_METHOD: u16 = 0;
const ZIP_DEFLATE_METHOD: u16 = 8;
const ZIP_VERSION_NEEDED: u16 = 20;
const ZIP_VERSION_MADE_BY: u16 = 20;
const ZIP_SEARCH_BACK_MAX: usize = 65_557;
const ZIP_ENTRY_BYTES_MAX: usize = 512 * 1024 * 1024;

/// Write a `.gds3d` archive containing the editable project document.
#[cfg(test)]
pub fn write_project_archive(file_path: &Path, document: &ProjectDocument) -> anyhow::Result<()> {
    let path = normalize_output_path(file_path)?;
    let mut file = fs::File::create(&path)
        .with_context(|| format!("create project archive: {}", path.display()))?;
    write_project_archive_to(&mut file, document)
}

pub fn write_project_archive_to(
    file: &mut fs::File,
    document: &ProjectDocument,
) -> anyhow::Result<()> {
    let archive = DocumentArchive {
        format: "gds3d",
        version: DOCUMENT_ARCHIVE_FORMAT_VERSION,
        document,
    };
    let mut entries = vec![ZipEntry::from_json(
        SCENE_JSON_NAME,
        &serde_json::to_value(archive)?,
    )?];
    write_document_sources(&mut entries, document.sources.values())?;
    write_zip(file, &entries)
}

/// Read the current project archive.
pub fn read_project_document(file_path: &Path) -> anyhow::Result<ProjectDocument> {
    let entries = read_zip_file(file_path)?;
    let scene_data = entries
        .iter()
        .find(|entry| entry.name == SCENE_JSON_NAME)
        .ok_or_else(|| anyhow!("project archive is missing scene.json"))?;
    let value: Value = serde_json::from_slice(&scene_data.data).context("parse scene.json")?;
    let version = value
        .get("version")
        .or_else(|| value.get("format_version"))
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("project archive is missing a format version"))?;
    if version != u64::from(DOCUMENT_ARCHIVE_FORMAT_VERSION) {
        bail!("unsupported project archive version: {version}");
    }
    let archive: DocumentArchiveOwned =
        serde_json::from_value(value).context("parse document project archive")?;
    if archive.format != "gds3d" {
        bail!("unsupported project archive format: {}", archive.format);
    }
    if archive.version != DOCUMENT_ARCHIVE_FORMAT_VERSION {
        bail!("unsupported project archive version: {}", archive.version);
    }
    let mut document = archive.document;
    for source in document.sources.values_mut() {
        let entry_name = format!("{RAW_GDS_DIR}/{}", source.source_key);
        source.embedded_data = entries
            .iter()
            .find(|entry| entry.name == entry_name)
            .map(|entry| entry.data.clone());
    }
    Ok(document)
}

/// Write a `.gds3d` project archive with `scene.json` and embedded GDS sources.
fn write_document_sources<'a>(
    entries: &mut Vec<ZipEntry>,
    sources: impl Iterator<Item = &'a SourceDocument>,
) -> anyhow::Result<()> {
    for source in sources {
        let data = match fs::read(&source.file_path) {
            Ok(data) => data,
            Err(_) => source.embedded_data.clone().ok_or_else(|| {
                anyhow!("GDS source is unavailable: {}", source.file_path.display())
            })?,
        };
        if data.len() > ZIP_ENTRY_BYTES_MAX {
            bail!(
                "GDS source is too large to embed: {}",
                source.file_path.display()
            );
        }
        let entry_name = format!("{RAW_GDS_DIR}/{}", source.source_key);
        entries.push(ZipEntry::from_bytes(&entry_name, data)?);
    }
    Ok(())
}

pub fn source_key_for_path(path: &Path) -> String {
    let display_path = path.to_string_lossy();
    let mut hasher = Crc32Hasher::new();
    hasher.update(display_path.as_bytes());
    let digest = hasher.finalize();
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("GDS");
    let suffix = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!(".{}", extension.to_ascii_lowercase()))
        .unwrap_or_default();
    format!("{stem}-{digest:08x}{suffix}")
}

#[cfg(test)]
fn normalize_output_path(file_path: &Path) -> anyhow::Result<PathBuf> {
    if file_path.extension().and_then(|suffix| suffix.to_str()) == Some("gds3d") {
        return Ok(file_path.to_path_buf());
    }
    bail!("project archive requires .gds3d suffix");
}

fn safe_archive_name(name: &str) -> bool {
    !name.contains('\\') && !name.contains("..") && !name.starts_with('/')
}

#[derive(Serialize)]
struct DocumentArchive<'a> {
    format: &'static str,
    version: u32,
    document: &'a ProjectDocument,
}

#[derive(Deserialize)]
struct DocumentArchiveOwned {
    format: String,
    version: u32,
    document: ProjectDocument,
}

#[derive(Clone)]
struct ZipEntry {
    name: String,
    method: u16,
    crc32: u32,
    compressed: Vec<u8>,
    uncompressed_size: u32,
}

impl ZipEntry {
    fn from_json(name: &str, value: &Value) -> anyhow::Result<Self> {
        Self::from_bytes(name, serde_json::to_vec(value)?)
    }

    fn from_bytes(name: &str, data: Vec<u8>) -> anyhow::Result<Self> {
        if !safe_archive_name(name) || name.is_empty() {
            bail!("invalid ZIP entry name: {name}");
        }
        if data.len() > ZIP_ENTRY_BYTES_MAX {
            bail!("ZIP entry too large: {name}");
        }

        let mut crc32 = Crc32Hasher::new();
        crc32.update(&data);
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&data)?;
        Ok(Self {
            name: name.to_owned(),
            method: ZIP_DEFLATE_METHOD,
            crc32: crc32.finalize(),
            compressed: encoder.finish()?,
            uncompressed_size: u32::try_from(data.len())?,
        })
    }
}

fn write_zip(mut file: &mut fs::File, entries: &[ZipEntry]) -> anyhow::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    let mut central_directory = Vec::new();
    let mut offset = 0u64;

    for entry in entries {
        let name = entry.name.as_bytes();
        let name_len = u16::try_from(name.len()).context("ZIP entry name too long")?;
        let compressed_size =
            u32::try_from(entry.compressed.len()).context("ZIP compressed entry too large")?;
        let local_offset = offset;

        write_u32(&mut file, ZIP_LOCAL_HEADER_SIGNATURE)?;
        write_u16(&mut file, ZIP_VERSION_NEEDED)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, entry.method)?;
        write_u16(&mut file, 0)?;
        write_u16(&mut file, 0)?;
        write_u32(&mut file, entry.crc32)?;
        write_u32(&mut file, compressed_size)?;
        write_u32(&mut file, entry.uncompressed_size)?;
        write_u16(&mut file, name_len)?;
        write_u16(&mut file, 0)?;
        file.write_all(name)?;
        file.write_all(&entry.compressed)?;
        offset += 30 + u64::from(name_len) + u64::from(compressed_size);

        write_u32(&mut central_directory, ZIP_CENTRAL_HEADER_SIGNATURE)?;
        write_u16(&mut central_directory, ZIP_VERSION_MADE_BY)?;
        write_u16(&mut central_directory, ZIP_VERSION_NEEDED)?;
        write_u16(&mut central_directory, 0)?;
        write_u16(&mut central_directory, entry.method)?;
        write_u16(&mut central_directory, 0)?;
        write_u16(&mut central_directory, 0)?;
        write_u32(&mut central_directory, entry.crc32)?;
        write_u32(&mut central_directory, compressed_size)?;
        write_u32(&mut central_directory, entry.uncompressed_size)?;
        write_u16(&mut central_directory, name_len)?;
        write_u16(&mut central_directory, 0)?;
        write_u16(&mut central_directory, 0)?;
        write_u16(&mut central_directory, 0)?;
        write_u16(&mut central_directory, 0)?;
        write_u32(&mut central_directory, 0)?;
        write_u32(&mut central_directory, u32::try_from(local_offset)?)?;
        central_directory.extend_from_slice(name);
    }

    let central_dir_offset = offset;
    let central_dir_size = u64::try_from(central_directory.len())?;
    file.write_all(&central_directory)?;
    let entry_count = u16::try_from(entries.len()).context("ZIP entry count too large")?;

    write_u32(&mut file, ZIP_END_SIGNATURE)?;
    write_u16(&mut file, 0)?;
    write_u16(&mut file, 0)?;
    write_u16(&mut file, entry_count)?;
    write_u16(&mut file, entry_count)?;
    write_u32(&mut file, u32::try_from(central_dir_size)?)?;
    write_u32(&mut file, u32::try_from(central_dir_offset)?)?;
    write_u16(&mut file, 0)?;
    file.flush()?;
    Ok(())
}

struct ReadEntry {
    name: String,
    data: Vec<u8>,
}

fn read_zip_file(path: &Path) -> anyhow::Result<Vec<ReadEntry>> {
    let data = fs::read(path)?;
    let end_offset =
        find_end_of_central_directory(&data).ok_or_else(|| anyhow!("ZIP end record not found"))?;
    let central_dir_size = usize::try_from(read_u32(&data, end_offset + 12)?)?;
    let central_dir_offset = usize::try_from(read_u32(&data, end_offset + 16)?)?;
    let central_dir_end = central_dir_offset
        .checked_add(central_dir_size)
        .ok_or_else(|| anyhow!("invalid ZIP central directory size"))?;
    if central_dir_end > data.len() {
        bail!("ZIP central directory is out of bounds");
    }

    let mut entries = Vec::new();
    let mut cursor = central_dir_offset;
    while cursor < central_dir_end {
        if read_u32(&data, cursor)? != ZIP_CENTRAL_HEADER_SIGNATURE {
            bail!("invalid ZIP central directory entry");
        }
        let method = read_u16(&data, cursor + 10)?;
        let compressed_size = usize::try_from(read_u32(&data, cursor + 20)?)?;
        let uncompressed_size = usize::try_from(read_u32(&data, cursor + 24)?)?;
        if uncompressed_size > ZIP_ENTRY_BYTES_MAX {
            bail!("ZIP entry exceeds size limit");
        }
        let name_len = usize::from(read_u16(&data, cursor + 28)?);
        let extra_len = usize::from(read_u16(&data, cursor + 30)?);
        let comment_len = usize::from(read_u16(&data, cursor + 32)?);
        let local_offset = usize::try_from(read_u32(&data, cursor + 42)?)?;
        let name_start = cursor + 46;
        let name_end = checked_range_end(name_start, name_len, data.len())?;
        let name = String::from_utf8(data[name_start..name_end].to_vec())
            .context("ZIP entry name is not UTF-8")?;

        let data_start = local_entry_data_start(&data, local_offset)?;
        let data_end = checked_range_end(data_start, compressed_size, data.len())?;
        let compressed = &data[data_start..data_end];
        let payload = decompress_entry(method, compressed, uncompressed_size)?;
        entries.push(ReadEntry {
            name,
            data: payload,
        });

        cursor = checked_range_end(name_end, extra_len + comment_len, central_dir_end)?;
    }

    Ok(entries)
}

fn local_entry_data_start(data: &[u8], local_offset: usize) -> anyhow::Result<usize> {
    if read_u32(data, local_offset)? != ZIP_LOCAL_HEADER_SIGNATURE {
        bail!("invalid ZIP local header");
    }
    let name_len = usize::from(read_u16(data, local_offset + 26)?);
    let extra_len = usize::from(read_u16(data, local_offset + 28)?);
    checked_range_end(local_offset + 30, name_len + extra_len, data.len())
}

fn decompress_entry(
    method: u16,
    compressed: &[u8],
    uncompressed_size: usize,
) -> anyhow::Result<Vec<u8>> {
    let mut payload = Vec::with_capacity(uncompressed_size);
    match method {
        ZIP_STORE_METHOD => payload.extend_from_slice(compressed),
        ZIP_DEFLATE_METHOD => {
            let mut decoder = DeflateDecoder::new(compressed);
            decoder.read_to_end(&mut payload)?;
        }
        _ => bail!("unsupported ZIP compression method: {method}"),
    }
    if payload.len() != uncompressed_size {
        bail!("ZIP entry size mismatch");
    }
    Ok(payload)
}

fn checked_range_end(start: usize, len: usize, limit: usize) -> anyhow::Result<usize> {
    let end = start
        .checked_add(len)
        .ok_or_else(|| anyhow!("ZIP offset overflow"))?;
    if end > limit {
        bail!("ZIP entry is out of bounds");
    }
    Ok(end)
}

fn find_end_of_central_directory(data: &[u8]) -> Option<usize> {
    if data.len() < 22 {
        return None;
    }
    let start = data.len().saturating_sub(ZIP_SEARCH_BACK_MAX);
    let mut cursor = data.len() - 22;
    loop {
        if read_u32(data, cursor).ok()? == ZIP_END_SIGNATURE {
            return Some(cursor);
        }
        if cursor == start {
            return None;
        }
        cursor -= 1;
    }
}

fn write_u16<W: Write>(writer: &mut W, value: u16) -> anyhow::Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_u32<W: Write>(writer: &mut W, value: u32) -> anyhow::Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn read_u16(data: &[u8], offset: usize) -> anyhow::Result<u16> {
    let end = checked_range_end(offset, 2, data.len())?;
    let mut raw = [0u8; 2];
    raw.copy_from_slice(&data[offset..end]);
    Ok(u16::from_le_bytes(raw))
}

fn read_u32(data: &[u8], offset: usize) -> anyhow::Result<u32> {
    let end = checked_range_end(offset, 4, data.len())?;
    let mut raw = [0u8; 4];
    raw.copy_from_slice(&data[offset..end]);
    Ok(u32::from_le_bytes(raw))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use crate::archive::{read_project_document, source_key_for_path, write_project_archive};
    use crate::model::{ProjectDocument, inspect_gds_file};

    #[test]
    fn source_key_suffix() {
        let key = source_key_for_path(Path::new("models/AWG.gds"));
        assert!(key.starts_with("AWG-"));
        assert!(key.ends_with(".gds"));
    }

    #[test]
    fn document_archive_round_trip() {
        let temp_dir = std::env::temp_dir().join(format!("gds3d-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).expect("create temp dir");
        let archive_path = temp_dir.join("project.gds3d");
        let gds_path = Path::new("../models/AWG.gds");
        let info = inspect_gds_file(gds_path).expect("inspect sample GDS");
        let selection = info
            .cells
            .first()
            .and_then(|cell| cell.layers.first())
            .map(|layer| layer.selection.clone())
            .expect("sample GDS layer selection");
        let mut document =
            ProjectDocument::from_gds(gds_path, &[selection]).expect("import project document");
        for layer in document.layer_views.values_mut() {
            layer.display.min_z = -2.5;
            layer.display.max_z = 12.5;
        }
        let mut expected_cell_ids = document.cells.keys().copied().collect::<Vec<_>>();
        expected_cell_ids.sort_by_key(|id| id.0);
        let mut expected_shape_ids = document.source_map.keys().copied().collect::<Vec<_>>();
        expected_shape_ids.sort_by_key(|id| id.0);

        write_project_archive(&archive_path, &document).expect("write document archive");
        let loaded = read_project_document(&archive_path).expect("read document archive");

        let mut loaded_cell_ids = loaded.cells.keys().copied().collect::<Vec<_>>();
        loaded_cell_ids.sort_by_key(|id| id.0);
        let mut loaded_shape_ids = loaded.source_map.keys().copied().collect::<Vec<_>>();
        loaded_shape_ids.sort_by_key(|id| id.0);
        assert_eq!(loaded_cell_ids, expected_cell_ids);
        assert_eq!(loaded_shape_ids, expected_shape_ids);
        assert_eq!(loaded.layer_views.len(), document.layer_views.len());
        assert!(
            loaded
                .layer_views
                .values()
                .all(|layer| layer.display.min_z == -2.5 && layer.display.max_z == 12.5)
        );
        assert_eq!(
            loaded
                .compile_render_scene()
                .expect("compile loaded document")
                .layers
                .len(),
            1
        );
        fs::remove_dir_all(&temp_dir).expect("remove temp dir");
    }
}
