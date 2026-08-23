//! The filesystem errands every other module here needs, each one
//! reporting the path it failed on. `std::fs` errors say "No such file or
//! directory" and not which one, and this whole tool exists to tell
//! somebody exactly which file is not where they thought.

use std::path::Path;

pub fn create_dir_all(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|err| format!("cannot make {}: {err}", path.display()))
}

pub fn remove_dir_all(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(path).map_err(|err| format!("cannot clear {}: {err}", path.display()))
}

pub fn copy(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        create_dir_all(parent)?;
    }
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|err| format!("cannot copy {} to {}: {err}", from.display(), to.display()))
}

pub fn read_to_string(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|err| format!("cannot read {}: {err}", path.display()))
}

pub fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    std::fs::write(path, text).map_err(|err| format!("cannot write {}: {err}", path.display()))
}

/// Every file under `root`, handed to `on_file` one at a time.
///
/// Symlinks are not followed, in either direction: a pack directory is
/// somebody's downloads folder, and one link pointing back up it would
/// make this walk forever.
pub fn walk(root: &Path, on_file: &mut impl FnMut(&Path)) -> Result<(), String> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .map_err(|err| format!("cannot list {}: {err}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("cannot list {}: {err}", dir.display()))?;
            let kind = entry
                .file_type()
                .map_err(|err| format!("cannot tell what {} is: {err}", entry.path().display()))?;
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                stack.push(entry.path());
            } else {
                on_file(&entry.path());
            }
        }
    }
    Ok(())
}
