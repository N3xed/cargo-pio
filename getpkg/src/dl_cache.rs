use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;
use url::Url;

use crate::progress::InstallProgress;

/// All errors returned from [`DlCache`].
#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to create file {path:?}")]
    FileCreateFailed { path: PathBuf, source: io::Error },
    #[error("file download request to '{url}' failed")]
    DownloadRequestFailed { url: String, source: ureq::Error },
    #[error("file download of '{url}' to {path:?} failed")]
    DownloadFailed {
        url: String,
        path: PathBuf,
        source: io::Error,
    },
}

/// Reuse downloaded files.
pub struct DlCache<'a> {
    dir: PathBuf,
    progress: Option<&'a dyn InstallProgress>,
}

impl DlCache<'_> {
    /// Create a new download cache that uses `dir` to save the downloaded files.
    pub(crate) fn new<'a>(dir: PathBuf, progress: Option<&'a dyn InstallProgress>) -> DlCache<'a> {
        DlCache { dir, progress }
    }

    /// Get the path to `file_name` if it exists in the download cache directory.
    pub fn get(&self, file_name: impl AsRef<OsStr>) -> Option<PathBuf> {
        let file = self.dir.join(file_name.as_ref());
        if file.exists() {
            Some(file)
        } else {
            None
        }
    }

    /// Create a new file if it doesn't exist or truncate the file if it does.
    pub fn get_file_truncated(
        &self,
        file_name: impl AsRef<Path>,
    ) -> Result<(File, PathBuf), Error> {
        let path = self.dir.join(file_name);
        Ok((
            std::fs::File::create(&path).map_err(|e| Error::FileCreateFailed {
                path: path.clone(),
                source: e,
            })?,
            path,
        ))
    }

    /// Get a cached file with `file_name` that already exists or download it from `url`.
    pub fn get_or_download(
        &self,
        url: String,
        file_name: impl AsRef<Path>,
    ) -> Result<PathBuf, Error> {
        if let Some(f) = self.get(file_name.as_ref()) {
            return Ok(f);
        }
        let mut file_name = file_name.as_ref().to_owned();
        if let Some(url_ext) = extract_url_file_extension(&url) {
            let file_ext = file_name.extension();
            match file_ext {
                Some(file_ext) if url_ext == file_ext => (),
                None => {
                    file_name.set_extension(url_ext);
                }
                _ => (),
            }
        }

        let (mut file, path) = self.get_file_truncated(file_name)?;

        let req = ureq::get(&url)
            .call()
            .map_err(|e| Error::DownloadRequestFailed {
                url: url.clone(),
                source: e,
            })?;

        if let Some(install_progress) = self.progress {
            let content_length = req
                .header("content-length")
                .and_then(|v| v.parse::<u64>().ok());
            let pb = install_progress.download(&url, &path, content_length);

            let mut reader = pb.wrap_read(req.into_reader());
            io::copy(&mut reader, &mut file)
        } else {
            let mut reader = req.into_reader();
            io::copy(&mut reader, &mut file)
        }
        .map_err(|e| Error::DownloadFailed {
            path: path.clone(),
            url,
            source: e,
        })?;

        Ok(path)
    }
}

fn extract_url_file_extension(url: &str) -> Option<OsString> {
    let url = Url::parse(url).ok()?;
    url.path_segments()
        .and_then(|s| s.last())
        .and_then(|p| Path::new(p).extension())
        .map(OsStr::to_owned)
}
