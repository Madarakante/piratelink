//! Launching console ROMs through an external emulator.
//!
//! A Switch dump is not an executable: the row stores the `.nsp`/`.xci` path,
//! and the launcher has to find the emulator that opens it. Detection is best
//! effort and ordered - the configured path wins, then the usual install
//! locations, then a shallow scan of the drive that holds the ROM, because a
//! portable Ryujinx usually sits next to the games (`D:\ryujinx-stable-...`).

use std::path::{Path, PathBuf};
use std::process::{Child, Command};

/// Executable names the Ryujinx family ships.
const EXE_NAMES: &[&str] = &["Ryujinx.exe", "Ryubing.exe"];

/// Folders that never contain an emulator and are expensive to walk.
const SKIP_DIRS: &[&str] = &[
    "windows",
    "$recycle.bin",
    "system volume information",
    "recovery",
    "perflogs",
    "config.msi",
    "node_modules",
    ".git",
];

/// Finds the Switch emulator: configured path first, then a quick search.
///
/// `rom` is optional because the UI also asks "do we have an emulator?" before a
/// ROM has been chosen.
pub fn find(configured: Option<&Path>, rom: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = configured.filter(|path| path.is_file()) {
        return Some(path.to_path_buf());
    }

    for candidate in known_locations() {
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    for root in search_roots(rom) {
        if let Some(found) = scan(&root, 2) {
            return Some(found);
        }
    }

    None
}

/// Starts the emulator with the ROM as its only argument.
pub fn launch(emulator: &Path, rom: &Path) -> std::io::Result<Child> {
    Command::new(emulator)
        .arg(rom)
        .current_dir(emulator.parent().unwrap_or(Path::new(".")))
        .spawn()
}

/// Candidate install locations: portable builds unpack next to a profile folder
/// surprisingly often, so the useful parents are checked directly.
fn known_locations() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for var in [
        "LOCALAPPDATA",
        "APPDATA",
        "PROGRAMFILES",
        "PROGRAMFILES(X86)",
        "USERPROFILE",
    ] {
        let Some(base) = std::env::var_os(var) else {
            continue;
        };
        let base = PathBuf::from(base);
        for folder in ["Ryujinx", "Ryubing"] {
            for exe in EXE_NAMES {
                out.push(base.join(folder).join(exe));
            }
        }
    }
    out
}

/// Drives worth scanning: the one holding the ROM, then the system drive.
fn search_roots(rom: Option<&Path>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    if let Some(drive) = rom.and_then(|rom| rom.ancestors().find(|path| path.parent().is_none())) {
        out.push(drive.to_path_buf());
    }
    if let Some(system) = std::env::var_os("SystemDrive") {
        let root = PathBuf::from(format!("{}\\", system.to_string_lossy()));
        out.push(root);
    }

    out.dedup();
    out
}

/// Depth-limited walk for an emulator executable. Returns the first hit.
fn scan(root: &Path, depth: usize) -> Option<PathBuf> {
    if depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(root).ok()?;

    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if path.is_dir() {
            if name.starts_with('$') || SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            dirs.push(path);
        } else if EXE_NAMES.iter().any(|exe| exe.to_lowercase() == name) {
            return Some(path);
        }
    }

    for dir in dirs {
        if let Some(found) = scan(&dir, depth - 1) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_finds_the_emulator_one_level_down() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("ryujinx-stable-1.3.3-windows_x64");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("Ryujinx.exe");
        std::fs::write(&exe, b"MZ").unwrap();

        assert_eq!(scan(tmp.path(), 2).as_deref(), Some(exe.as_path()));
    }

    #[test]
    fn scan_ignores_system_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let windows = tmp.path().join("Windows");
        std::fs::create_dir_all(&windows).unwrap();
        std::fs::write(windows.join("Ryujinx.exe"), b"MZ").unwrap();

        assert!(scan(tmp.path(), 3).is_none());
    }

    #[test]
    fn configured_path_wins_when_it_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("Ryujinx.exe");
        std::fs::write(&exe, b"MZ").unwrap();

        let found = find(Some(&exe), Some(Path::new(r"D:\Switch\game.nsp")));
        assert_eq!(found.as_deref(), Some(exe.as_path()));
    }
}
