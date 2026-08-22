use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use crate::archive;
use crate::model::ProjectDocument;

pub struct LockedProjectFile {
    path: PathBuf,
    file: File,
}

impl LockedProjectFile {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if !path.is_file() {
            bail!("project file does not exist: {}", path.display());
        }
        Self::open_with_options(path, false)
    }

    pub fn create(path: &Path) -> anyhow::Result<Self> {
        Self::validate_path(path)?;
        Self::open_with_options(path, true)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write(&mut self, document: &ProjectDocument) -> anyhow::Result<()> {
        if !self.path.is_file() {
            bail!("project file was removed: {}", self.path.display());
        }
        archive::write_project_archive_to(&mut self.file, document)?;
        self.file.sync_all().context("sync project archive")
    }

    fn open_with_options(path: &Path, create: bool) -> anyhow::Result<Self> {
        Self::validate_path(path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(create);
        configure_sharing(&mut options);
        let file = options
            .open(path)
            .with_context(|| format!("open project file: {}", path.display()))?;
        file.try_lock()
            .map_err(anyhow::Error::from)
            .with_context(|| format!("project file is already in use: {}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }

    fn validate_path(path: &Path) -> anyhow::Result<()> {
        if path.extension().and_then(|suffix| suffix.to_str()) != Some("gds3d") {
            bail!("project archive requires .gds3d suffix");
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn configure_sharing(options: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_SHARE_READ: u32 = 0x0000_0001;
    options.share_mode(FILE_SHARE_READ);
}

#[cfg(not(target_os = "windows"))]
fn configure_sharing(_options: &mut OpenOptions) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_second_lock() {
        let path = std::env::temp_dir().join(format!("gds3d-lock-{}.gds3d", uuid::Uuid::new_v4()));
        let first = LockedProjectFile::create(&path).expect("first project lock");
        assert!(LockedProjectFile::open(&path).is_err());
        drop(first);
        std::fs::remove_file(path).expect("remove lock fixture");
    }
}
