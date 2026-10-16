//! Atomic configuration persistence (Phase 2, Module C).
//!
//! Writes go to a temporary file in the destination directory and are then
//! `rename(2)`-ed over the target, so a crash or full disk mid-write can never
//! leave a truncated/corrupt `config.toml`.

use std::io::Write;
use std::path::Path;

/// Atomically replace `path` with `contents`.
///
/// The parent directory is created when missing. The temporary file is created
/// in the same directory as the target so the final rename stays on one
/// filesystem (a cross-device rename is not atomic).
pub fn atomic_write(path: &Path, contents: &str) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;

    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(contents.as_bytes())?;
    temp.flush()?;
    temp.as_file().sync_all()?;

    temp.persist(path).map_err(|err| err.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_replaces_atomically() {
        let dir = std::env::temp_dir().join(format!(
            "paneship-persist-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let target = dir.join("nested").join("config.toml");

        atomic_write(&target, "a = 1\n").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "a = 1\n");

        atomic_write(&target, "a = 2\n").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "a = 2\n");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
