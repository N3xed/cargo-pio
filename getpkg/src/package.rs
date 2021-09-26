use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use enumflags2::{bitflags, BitFlags};
use serde::{Deserialize, Serialize};

use crate::index::InstallContext;

/// An operating system.
#[bitflags]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Os {
    Linux,
    Macos,
    Windows,
}

/// A hardware architecture.
#[bitflags]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Arch {
    Arm64,
    Arm32,
    X86_64,
    X86,
}

/// A platform; operating system and supported architectures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(packed)]
pub struct Platform(pub Os, pub BitFlags<Arch>);

/// A collection of installable package versions.
pub trait PackageSource {
    type Package: Package;

    fn name(&self) -> String;
    fn description(&self) -> String;

    /// Get a collection of all version strings ordered from latest to oldest.
    fn versions(&self) -> Vec<String>;
    /// Get the latest version string.
    fn latest_version(&self) -> String;

    /// Get the latest package where `version` matches the beginning of its version string
    /// and it supports all platforms in `platforms`.
    fn package(&self, version: &str, platforms: &[Platform]) -> Self::Package;
}

/// Metadata about an installed package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    /// The name of this package.
    pub name: String,
    /// The version of this package.
    pub version: String,
    /// One or more environment variables that this package exports.
    #[serde(default)]
    pub exported_env_vars: HashMap<OsString, OsString>,
    /// One or more binary directories that should be added to `PATH`.
    ///
    /// These paths are relative to the folder where this tool is installed.
    #[serde(default)]
    pub bin_dirs: Vec<PathBuf>,
    /// The absolute path to the folder where this tool is installed.
    ///
    /// It is equal to `path` given to [`Package::install_at`] that initially produced
    /// this [`PackageMetadata`].
    pub path: PathBuf,
}

/// A specific package that can be installed.
pub trait Package {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Install this package in the directory `path` and get the metadata of the installed tool.
    fn install_at(
        &mut self,
        path: &Path,
        ctx: InstallContext,
    ) -> Result<PackageMetadata, Self::Error>;

    /// The version of this package.
    fn version(&self) -> String;
    /// The name of this package.
    fn name(&self) -> String;
    /// The description of this package.
    fn description(&self) -> String;

    /// Get a displayable name of this package.
    fn display_name(&self) -> String {
        format!("{}@{}", self.name(), self.version())
    }
}
