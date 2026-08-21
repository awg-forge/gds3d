use flate2::read::GzDecoder;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use tar::Archive;
use tauri::{AppHandle, Emitter, Manager};
use tempfile::NamedTempFile;
use zip::ZipArchive;

use super::{FrpDownloadProgress, FrpProvider, PROGRESS_EVENT};
use crate::download::{self, DownloadRequest};

const MAX_BYTES: u64 = 64 * 1024 * 1024;

pub(super) struct ClientDownload {
    pub(super) urls: Vec<String>,
    pub(super) archive: ArchiveKind,
    pub(super) expected_size: Option<u64>,
    pub(super) expected_md5: Option<String>,
}

pub(super) enum ArchiveKind {
    Raw,
    Zip,
    TarGz,
}

fn filename(provider: FrpProvider) -> &'static str {
    match (provider, cfg!(windows)) {
        (FrpProvider::OpenFrp, true) => "openfrpc.exe",
        (FrpProvider::OpenFrp, false) => "openfrpc",
        (FrpProvider::SakuraFrp, true) => "sakurafrpc.exe",
        (FrpProvider::SakuraFrp, false) => "sakurafrpc",
    }
}

pub(super) fn directory(app: &AppHandle, provider: FrpProvider) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|path| path.join("frp").join(provider.directory()))
        .map_err(|error| error.to_string())
}

pub(super) fn path(app: &AppHandle, provider: FrpProvider) -> Result<PathBuf, String> {
    Ok(directory(app, provider)?.join(filename(provider)))
}

pub(super) async fn install(app: &AppHandle, provider: FrpProvider) -> Result<(), String> {
    let directory = directory(app, provider)?;
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let item = match provider {
        FrpProvider::OpenFrp => super::openfrp::client().await?,
        FrpProvider::SakuraFrp => super::sakurafrp::client().await?,
    };
    log::info!("resolved {} client manifest", provider.display_name());
    let archive_path = directory.join(".client-download");
    let progress_app = app.clone();
    download::download(
        DownloadRequest {
            urls: item.urls.clone(),
            destination: archive_path.clone(),
            expected_size: item.expected_size,
            max_size: MAX_BYTES,
            concurrency: 6,
        },
        move |progress| {
            emit_progress(
                &progress_app,
                provider,
                progress.downloaded_bytes,
                progress.total_bytes,
            );
        },
    )
    .await
    .map_err(|error| format!("client download failed: {error}"))?;
    log::info!("downloaded {} client archive", provider.display_name());
    let bytes = fs::read(&archive_path);
    let _ = fs::remove_file(&archive_path);
    let bytes = bytes.map_err(|error| format!("failed to read client archive: {error}"))?;
    verify(&bytes, &item).map_err(|error| format!("client verification failed: {error}"))?;

    let executable = extract(&bytes, item.archive)
        .map_err(|error| format!("client extraction failed: {error}"))?;
    let target = directory.join(filename(provider));
    write_binary(&directory, &target, &executable)
        .map_err(|error| format!("failed to install client executable: {error}"))?;
    log::info!(
        "installed {} client at {}",
        provider.display_name(),
        target.display()
    );
    Ok(())
}

fn emit_progress(
    app: &AppHandle,
    provider: FrpProvider,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) {
    let percent = total_bytes
        .filter(|total| *total > 0)
        .map(|total| ((downloaded_bytes.saturating_mul(100) / total).min(100)) as u8)
        .unwrap_or(0);
    let progress = FrpDownloadProgress {
        provider,
        downloaded_bytes,
        total_bytes,
        percent,
    };
    if let Err(error) = app.emit(PROGRESS_EVENT, progress) {
        log::warn!("failed to emit FRP download progress: {error}");
    }
}

pub(super) fn platform() -> Result<(&'static str, &'static str), String> {
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        return Err("the current operating system is not supported".to_owned());
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "386"
    } else {
        return Err("the current CPU architecture is not supported".to_owned());
    };
    Ok((os, arch))
}

fn verify(bytes: &[u8], item: &ClientDownload) -> Result<(), String> {
    if let Some(size) = item.expected_size
        && bytes.len() as u64 != size
    {
        return Err("downloaded client size does not match the manifest".to_owned());
    }
    if let Some(expected) = item.expected_md5.as_deref() {
        let actual = format!("{:x}", md5::compute(bytes));
        if !actual.eq_ignore_ascii_case(expected) {
            return Err("downloaded client checksum does not match the manifest".to_owned());
        }
    }
    Ok(())
}

fn extract(bytes: &[u8], archive: ArchiveKind) -> Result<Vec<u8>, String> {
    match archive {
        ArchiveKind::Raw => Ok(bytes.to_vec()),
        ArchiveKind::Zip => {
            let mut archive =
                ZipArchive::new(Cursor::new(bytes)).map_err(|error| error.to_string())?;
            for index in 0..archive.len() {
                let mut file = archive.by_index(index).map_err(|error| error.to_string())?;
                if file.is_file() && is_frpc(file.name()) {
                    let mut executable = Vec::new();
                    file.read_to_end(&mut executable)
                        .map_err(|error| error.to_string())?;
                    return Ok(executable);
                }
            }
            Err("client archive does not contain frpc".to_owned())
        }
        ArchiveKind::TarGz => {
            let decoder = GzDecoder::new(Cursor::new(bytes));
            let mut archive = Archive::new(decoder);
            for entry in archive.entries().map_err(|error| error.to_string())? {
                let mut entry = entry.map_err(|error| error.to_string())?;
                let path = entry.path().map_err(|error| error.to_string())?;
                if entry.header().entry_type().is_file() && is_frpc(&path.to_string_lossy()) {
                    let mut executable = Vec::new();
                    entry
                        .read_to_end(&mut executable)
                        .map_err(|error| error.to_string())?;
                    return Ok(executable);
                }
            }
            Err("client archive does not contain frpc".to_owned())
        }
    }
}

fn is_frpc(path: &str) -> bool {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            let name = name.to_ascii_lowercase();
            name == "frpc" || name == "frpc.exe" || name.starts_with("frpc_")
        })
}

fn write_binary(directory: &Path, target: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut temporary = NamedTempFile::new_in(directory).map_err(|error| error.to_string())?;
    temporary
        .write_all(bytes)
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o755))
            .map_err(|error| error.to_string())?;
    }
    temporary
        .persist(target)
        .map_err(|error| error.error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;

    #[test]
    fn recognizes_frpc() {
        assert!(is_frpc("release/frpc"));
        assert!(is_frpc("release/frpc.exe"));
        assert!(is_frpc("frpc_windows_amd64.exe"));
        assert!(!is_frpc("release/frps"));
    }

    #[test]
    fn names_clients() {
        let extension = if cfg!(windows) { ".exe" } else { "" };
        assert_eq!(
            filename(FrpProvider::OpenFrp),
            format!("openfrpc{extension}")
        );
        assert_eq!(
            filename(FrpProvider::SakuraFrp),
            format!("sakurafrpc{extension}")
        );
    }

    #[test]
    fn extracts_provider_binary() {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut archive = zip::ZipWriter::new(&mut bytes);
            archive
                .start_file("frpc_windows_amd64.exe", SimpleFileOptions::default())
                .unwrap();
            archive.write_all(b"frpc binary").unwrap();
            archive.finish().unwrap();
        }

        let binary = extract(bytes.get_ref(), ArchiveKind::Zip).unwrap();
        assert_eq!(binary, b"frpc binary");

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join(filename(FrpProvider::OpenFrp));
        write_binary(directory.path(), &target, &binary).unwrap();
        assert_eq!(fs::read(target).unwrap(), b"frpc binary");
    }
}
