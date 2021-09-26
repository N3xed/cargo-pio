use std::ffi::OsString;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::dl_cache::DlCache;
use crate::progress::{InstallProgress, Progress};
use crate::{Package, PackageMetadata};

const DEFAULT_INDEX_FILE: &str = "getpkg.json";
const DEFAULT_DLCACHE_DIR: &str = "dlcache";

/// An error returned from methods of [`PackageIndex`].
#[derive(Error, Debug)]
pub enum Error {
    #[error("installed packages index {0:?} not found")]
    IndexNotFound(PathBuf),
    #[error("could not open installed packages index {path:?}")]
    IndexOpenFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("could not create installed packages index {path:?}")]
    IndexCreateFailed {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("writing installed packages index to {path:?} failed")]
    SerializationFailed {
        path: PathBuf,
        source: anyhow::Error,
    },
    #[error("reading installed packages index from {path:?} failed")]
    DeserializationFailed {
        path: PathBuf,
        source: anyhow::Error,
    },
    #[error("could not create install directory {path:?} for package '{package}'")]
    CreateInstallDirFailed {
        path: PathBuf,
        package: String,
        source: anyhow::Error,
    },
    #[error("could not install package '{package}' at {path:?}")]
    InstallFailed {
        path: PathBuf,
        package: String,
        source: anyhow::Error,
    },
    #[error("could not create a relative path from {from:?} to {to:?}")]
    InvalidRelativePath { from: PathBuf, to: PathBuf },
}

/// A collection of installed packages.
pub struct PackageIndex {
    /// The directory that contains the index of all installed packages.
    ///
    /// It is also used as the base directory that all installed tool's paths are relative
    /// to.
    dir: PathBuf,
    /// The file name of the index file.
    index_file: OsString,
    progress: Option<Box<dyn Progress>>,
    data: IndexData,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone, Debug)]
struct IndexData {
    cache_dir: Option<PathBuf>,
    /// The metadata of all installed packages.
    packages: Vec<PackageMetadata>,
}

impl PackageIndex {
    /// Create an empty package index.
    pub fn new(dir: impl Into<PathBuf>, index_file_name: impl Into<OsString>) -> PackageIndex {
        let dir = dir.into();
        PackageIndex {
            dir,
            index_file: index_file_name.into(),
            progress: None,
            data: IndexData::default(),
        }
    }

    /// Open a package index with the default index file if the index file exists
    /// otherwise create an empty index.
    pub fn open(dir: impl Into<PathBuf>) -> Result<PackageIndex, Error> {
        Self::open_with(dir, DEFAULT_INDEX_FILE)
    }

    /// Open a package index with `index_file_name` if the index file exists otherwise
    /// create an empty index.
    pub fn open_with(
        dir: impl Into<PathBuf>,
        index_file_name: impl Into<OsString>,
    ) -> Result<PackageIndex, Error> {
        let dir = dir.into();
        let index_file_name = index_file_name.into();
        match Self::load(&dir, &index_file_name) {
            Err(Error::IndexNotFound { .. }) => Ok(PackageIndex::new(dir, index_file_name)),
            r => r,
        }
    }

    pub fn set_progress_handler(&mut self, progress: impl Progress + 'static) {
        self.progress = Some(Box::new(progress));
    }

    /// Load a package index from the index file in `dir` named `index_file_name`.
    pub fn load(
        dir: impl Into<PathBuf>,
        index_file_name: impl Into<OsString>,
    ) -> Result<PackageIndex, Error> {
        let dir: PathBuf = dir.into();
        let index_file_name: OsString = index_file_name.into();

        let index_file = dir.join(&index_file_name);
        if !index_file.exists() {
            return Err(Error::IndexNotFound(index_file));
        }

        let index_fd = std::fs::File::open(&index_file).map_err(|e| Error::IndexOpenFailed {
            path: index_file.clone(),
            source: e,
        })?;

        let mut data: IndexData =
            serde_json::from_reader(index_fd).map_err(|e| Error::DeserializationFailed {
                path: index_file,
                source: anyhow::Error::new(e),
            })?;

        for p in data.packages.iter_mut() {
            p.path = dir.join(&p.path);
        }

        Ok(PackageIndex {
            index_file: index_file_name,
            dir,
            progress: None,
            data,
        })
    }

    /// Get metadata about an installed package matching `name` and `version`.
    pub fn get(&self, name: impl AsRef<str>, version: impl AsRef<str>) -> Option<&PackageMetadata> {
        let name = name.as_ref();
        let version = version.as_ref();
        self.data
            .packages
            .iter()
            .find(|t| t.name == name && t.version == version)
    }

    /// Get metadata about installed packages matching `name`.
    pub fn get_by_name(&self, name: impl Into<String>) -> impl Iterator<Item = &PackageMetadata> {
        let name = name.into();
        self.data.packages.iter().filter(move |t| t.name == name)
    }

    /// Install a package in the directory `path` relative to this index if it isn't
    /// installed already and return its metadata.
    pub fn install_at(
        &mut self,
        package: &mut impl Package,
        path: impl AsRef<Path>,
    ) -> Result<PackageMetadata, Error> {
        let name = package.name();
        let version = package.version();
        let path = self.dir.join(path.as_ref());

        let package_match = self.get(&name, &version).and_then(|p| {
            match (p.path.canonicalize(), path.canonicalize()) {
                (Ok(p0), Ok(p1)) if p0 == p1 => Some(p),
                _ => None,
            }
        });
        if let Some(t) = package_match {
            Ok(t.clone())
        } else {
            std::fs::create_dir_all(&path).map_err(|e| Error::CreateInstallDirFailed {
                package: package.display_name(),
                path: path.clone(),
                source: e.into(),
            })?;

            let install_context = InstallContext {
                progress: self.progress.as_ref().map(|p| p.install(&name, &path)),
                index: self,
            };

            let metadata =
                package
                    .install_at(&path, install_context)
                    .map_err(|err| Error::InstallFailed {
                        package: package.display_name(),
                        path,
                        source: err.into(),
                    })?;

            self.data.packages.push(metadata.clone());
            Ok(metadata)
        }
    }

    pub fn dlcache<'a>(&self, install_progress: Option<&'a dyn InstallProgress>) -> DlCache<'a> {
        let cache_dir = self.dir.join(
            self.data
                .cache_dir
                .clone()
                .unwrap_or_else(|| DEFAULT_DLCACHE_DIR.into()),
        );
        DlCache::new(cache_dir, install_progress)
    }

    /// Install a package in the subfolder `<pkgname>-<pkgversion>` of this index if it
    /// isn't installed already and return its metadata.
    pub fn install(&mut self, package: &mut impl Package) -> Result<PackageMetadata, Error> {
        self.install_at(
            package,
            &format!("{}-{}", package.name(), package.version()),
        )
    }

    /// Save all metadata about the installed packages to the index file.
    pub fn save(&self) -> Result<(), Error> {
        let mut data = self.data.clone();
        // Make package paths relative to `self.dir`.
        for p in data.packages.iter_mut() {
            p.path = pathdiff::diff_paths(&p.path, &self.dir).ok_or_else(|| {
                Error::InvalidRelativePath {
                    from: self.dir.clone(),
                    to: p.path.clone(),
                }
            })?;
        }

        let index_file = self.dir.join(&self.index_file);
        let index_fd =
            std::fs::File::create(&index_file).map_err(|e| Error::IndexCreateFailed {
                path: index_file.clone(),
                source: e,
            })?;

        serde_json::to_writer_pretty(index_fd, &data).map_err(|e| Error::SerializationFailed {
            path: index_file,
            source: e.into(),
        })?;
        Ok(())
    }
}

impl Drop for PackageIndex {
    /// Save the package index to file.
    fn drop(&mut self) {
        self.save().ok();
    }
}

pub struct InstallContext<'a> {
    index: &'a mut PackageIndex,
    progress: Option<Box<dyn InstallProgress>>,
}

impl InstallContext<'_> {
    /// Get the index for this installation.
    pub fn index(&self) -> &PackageIndex {
        self.index
    }

    /// Get the index mutable for this installation.
    pub fn index_mut(&mut self) -> &mut PackageIndex {
        self.index
    }

    /// Get the download cache of the index for this installation.
    pub fn dlcache(&self) -> DlCache {
        self.index.dlcache(self.progress.as_deref())
    }
}
