//! Progress tracking.

use std::path::Path;

pub struct ProgressBar {
    pb: indicatif::ProgressBar,
    on_finish: Option<Box<dyn FnOnce(&indicatif::ProgressBar) + Send>>,
}

impl ProgressBar {
    /// Create a new progress bar from `pb` that uses `on_finish` to finish it.
    pub fn new(
        pb: indicatif::ProgressBar,
        on_finish: impl FnOnce(&indicatif::ProgressBar) + Send + 'static,
    ) -> ProgressBar {
        ProgressBar {
            pb,
            on_finish: Some(Box::new(on_finish)),
        }
    }

    /// Create a new progress bar from `pb` that uses
    /// [`indicatif::ProgressBar::finish_using_style`] to finish it.
    pub fn new_using_style(pb: indicatif::ProgressBar) -> ProgressBar {
        Self::new(pb, |pb| pb.finish_using_style())
    }

    /// Finish the progress bar using the specified closure on creation.
    pub fn finish(&mut self) {
        if let Some(on_finish) = self.on_finish.take() {
            on_finish(&self.pb);
        }
    }
}

impl std::ops::Deref for ProgressBar {
    type Target = indicatif::ProgressBar;

    fn deref(&self) -> &Self::Target {
        &self.pb
    }
}

impl std::ops::DerefMut for ProgressBar {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.pb
    }
}

impl Drop for ProgressBar {
    /// Finish the progress bar if it isn't already.
    fn drop(&mut self) {
        self.finish();
    }
}

pub trait InstallProgress: Send + Sync {
    /// Create a progress bar that tracks the progress of a download.
    fn download(&self, url: &str, dest_file: &Path, size_bytes: Option<u64>) -> ProgressBar;
}

/// Track the progress of [`PackageIndex`](crate::PackageIndex) operations.
pub trait Progress {
    /// Create a [`InstallProgress`] that tracks the progress of a package installation.
    fn install(&self, package_name: &str, dir: &Path) -> Box<dyn InstallProgress>;
}