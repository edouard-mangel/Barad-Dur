use std::path::PathBuf;

use anyhow::Result;

/// A cloned repository that lives in a temporary directory.
/// The directory is automatically removed when this value is dropped.
pub struct TempClone {
    pub path: PathBuf,
    _dir: tempfile::TempDir,
}

/// Clone a remote git URL into a fresh temporary directory and return a
/// [`TempClone`] handle. The clone is removed from disk when the handle is
/// dropped.
pub fn clone_remote(url: &str) -> Result<TempClone> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().to_path_buf();
    eprintln!("Cloning {}...", url);
    git2::Repository::clone(url, &path)?;
    Ok(TempClone { path, _dir: dir })
}
