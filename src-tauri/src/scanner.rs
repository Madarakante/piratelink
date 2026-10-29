//! Directory scanning.
//!
//! Deliberately a *suggester*, not an auto-classifier: it ranks executables and
//! explains why, and the UI asks the user to confirm. A launcher that silently
//! decides "this file is a game" gets it wrong in both directions — it misses
//! non-Steam layouts and it happily matches redistributables.

use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;

const MAX_DEPTH: usize = 4;

/// Path components that never contain the thing the user double-clicks.
const NOISE_DIRS: &[&str] = &[
    "_commonredist",
    "redist",
    "redistributables",
    "directx",
    "dotnet",
    "vcredist",
    "installers",
    "installer",
    "easyanticheat",
    "battleye",
    "support",
    "docs",
    "tools",
    "editor",
    "crashreportclient",
];

/// Executable names that are installers, crash handlers or runtimes.
const NOISE_STEMS: &[&str] = &[
    "unins",
    "uninstall",
    "vc_redist",
    "vcredist",
    "dxsetup",
    "dxwebsetup",
    "dotnetfx",
    "oalinst",
    "crashreport",
    "crashhandler",
    "unitycrashhandler",
    "unitycrashhandler64",
    "ue4prereqsetup",
    "ue5prereqsetup",
    "easyanticheat",
    "eac_launcher",
    "beservice",
    "steamsetup",
    "steamclient_loader",
    "7z",
    "winrar",
    "python",
    "node",
    "chrome",
    "firefox",
];

const MIN_SCORE: i32 = 3;
const MAX_RESULTS: usize = 300;

#[derive(Debug, Clone, Serialize)]
pub struct Candidate {
    pub path: String,
    pub install_dir: String,
    /// Best-effort title guess, used to prefill the SteamGridDB search box.
    pub name_hint: String,
    pub score: i32,
    /// Human-readable reasons, shown next to each suggestion.
    pub reasons: Vec<String>,
    pub size_bytes: u64,
    pub already_in_library: bool,
}

/// Walks `root` and returns ranked executable candidates.
///
/// `known` holds canonical executable paths already in the library, so the UI can
/// grey them out instead of re-adding them.
pub fn scan_root(root: &Path, known: &HashSet<String>) -> Vec<Candidate> {
    let mut found = Vec::new();
    walk(root, root, 0, known, &mut found);

    // A junction or symlink can surface the same executable twice. Duplicates
    // would land in a keyed list in the UI, so collapse them here.
    let mut seen = HashSet::new();
    found.retain(|candidate| seen.insert(candidate.path.to_lowercase()));

    found.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.size_bytes.cmp(&a.size_bytes))
    });
    found.truncate(MAX_RESULTS);
    found
}

fn walk(root: &Path, dir: &Path, depth: usize, known: &HashSet<String>, out: &mut Vec<Candidate>) {
    if depth > MAX_DEPTH {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        // Unreadable directories (permissions, severed junctions) are skipped, not
        // fatal: one bad folder must not abort a whole library scan.
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };

        // Never follow symlinks or junctions: a self-referencing link would loop.
        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            if is_noise_dir(&path) {
                continue;
            }
            walk(root, &path, depth + 1, known, out);
            continue;
        }

        let is_exe = path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("exe"))
            .unwrap_or(false);
        if !is_exe {
            continue;
        }

        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let Some(candidate) = evaluate(&path, depth, size_bytes, known) else {
            continue;
        };
        out.push(candidate);
    }
}

/// Directory names that describe a build layout, never a game.
///
/// The title has to come from the folder *above* these: `Elden Ring\Game\eldenring.exe`
/// lives in a folder called "Game", and searching SteamGridDB for "Game" returns a
/// game literally named GAME with completely unrelated artwork.
const STRUCTURAL_DIRS: &[&str] = &[
    "bin",
    "binaries",
    "x64",
    "x86",
    "win64",
    "win32",
    "game",
    "content",
    "engine",
    "retail",
    "shipping",
    "common",
    "steamapps",
    "steamlibrary",
    "gog games",
    "epic games",
    "program files",
    "program files (x86)",
    "users",
    "desktop",
    "downloads",
    "library",
    "media",
    "data",
    "build",
    "dist",
];

/// Walks up from the executable until it finds a folder that could be a game name.
fn title_folder(exe: &Path) -> Option<String> {
    let mut current = exe.parent()?;

    // Bounded: four levels is deeper than any sane layout, and stops a stray walk
    // from climbing into the volume root.
    for _ in 0..4 {
        let name = current.file_name()?.to_string_lossy().to_string();
        let lowered = name.trim().to_lowercase();
        if !lowered.is_empty() && !STRUCTURAL_DIRS.contains(&lowered.as_str()) {
            return Some(name);
        }
        current = current.parent()?;
    }

    exe.parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
}

fn evaluate(
    exe: &Path,
    depth: usize,
    size_bytes: u64,
    known: &HashSet<String>,
) -> Option<Candidate> {
    let stem = exe.file_stem()?.to_string_lossy().to_string();
    let normalised_stem = normalise(&stem);

    let install_dir = exe.parent()?.to_path_buf();
    let folder_name = title_folder(exe).unwrap_or_default();
    let normalised_folder = normalise(&folder_name);

    let mut score = 0;
    let mut reasons = Vec::new();

    if NOISE_STEMS.iter().any(|n| normalised_stem.starts_with(n)) {
        return None;
    }
    if exe
        .components()
        .any(|c| NOISE_DIRS.contains(&c.as_os_str().to_string_lossy().to_lowercase().as_str()))
    {
        return None;
    }

    if normalised_folder.contains(&normalised_stem) || normalised_stem.contains(&normalised_folder)
    {
        score += 4;
        reasons.push("named after its folder".to_string());
    }

    if size_bytes >= 20 * 1024 * 1024 {
        score += 4;
        reasons.push(format!("{} MB binary", size_bytes / (1024 * 1024)));
    } else if size_bytes >= 5 * 1024 * 1024 {
        score += 2;
        reasons.push(format!("{} MB binary", size_bytes / (1024 * 1024)));
    } else if size_bytes < 128 * 1024 {
        score -= 4;
        reasons.push("very small executable".to_string());
    }

    if normalised_stem.ends_with("shipping") {
        score += 2;
        reasons.push("Unreal shipping build".to_string());
    }

    if depth <= 2 {
        score += 2;
        reasons.push("close to the root".to_string());
    }

    // A Steam stub next to the exe is a weak positive, never a classifier: plenty
    // of legitimately owned games have one, and plenty of games do not.
    if install_dir.join("steam_api64.dll").is_file() || install_dir.join("steam_api.dll").is_file()
    {
        score += 1;
        reasons.push("ships the Steam API stub".to_string());
    }

    if score < MIN_SCORE {
        return None;
    }

    let canonical = exe.canonicalize().unwrap_or_else(|_| exe.to_path_buf());
    let path = exe.to_string_lossy().to_string();

    Some(Candidate {
        already_in_library: known.contains(&canonical.to_string_lossy().to_string()),
        path,
        install_dir: install_dir.to_string_lossy().to_string(),
        name_hint: clean_title(&folder_name),
        score,
        reasons,
        size_bytes,
    })
}

fn is_noise_dir(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    NOISE_DIRS.contains(&name.as_str())
}

fn normalise(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

/// Turns a release folder into something worth typing into a search box.
///
/// `Hades_v1.38261.2_(59229)_[FitGirl Repack]` -> `Hades`
/// `Ori.and.the.Will.of.the.Wisps-CODEX`      -> `Ori and the Will of the Wisps`
pub fn clean_title(folder: &str) -> String {
    // Drop bracketed groups first; release groups live there.
    let mut text = String::new();
    let mut depth = 0usize;
    for ch in folder.chars() {
        match ch {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => text.push(ch),
            _ => {}
        }
    }

    let noise: &[&str] = &[
        "fitgirl", "repack", "codex", "plaza", "skidrow", "elamigos", "gog", "razor1911",
        "reloaded", "cpy", "goldberg", "online", "multiplayer", "portable", "win64", "win32",
        "update", "hotfix", "dlc", "rip", "proper", "cracked", "rus", "eng", "multi",
    ];

    let mut tokens: Vec<String> = Vec::new();
    for raw in text.split(['_', '.', '-', '+']) {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }
        let lower = token.to_lowercase();
        if noise.contains(&lower.as_str()) {
            continue;
        }
        // Version-like tokens: v1.2, 1.38261.2, 59229
        if lower.starts_with('v') && lower[1..].chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        if lower.chars().all(|c| c.is_ascii_digit() || c == '.') {
            continue;
        }
        tokens.push(token.to_string());
    }

    if tokens.is_empty() {
        return folder.trim().to_string();
    }

    tokens.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn write_exe(path: &Path, size: u64) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let file = File::create(path).unwrap();
        // Sparse file: the scanner only stats it, so the bytes never land on disk.
        file.set_len(size).unwrap();
    }

    const MB: u64 = 1024 * 1024;

    #[test]
    fn picks_the_game_and_ignores_installers() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        write_exe(&root.join("Hades").join("Hades.exe"), 40 * MB);
        write_exe(&root.join("Hades").join("unins000.exe"), 20 * MB);
        write_exe(&root.join("Hades").join("_CommonRedist").join("vcredist_x64.exe"), 30 * MB);
        write_exe(&root.join("Hades").join("UnityCrashHandler64.exe"), 10 * MB);

        let found = scan_root(root, &HashSet::new());
        let names: Vec<_> = found
            .iter()
            .map(|c| Path::new(&c.path).file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert!(names.contains(&"Hades.exe".to_string()), "got {names:?}");
        assert!(!names.iter().any(|n| n.starts_with("unins")), "got {names:?}");
        assert!(!names.iter().any(|n| n.contains("vcredist")), "got {names:?}");
        assert!(!names.iter().any(|n| n.contains("CrashHandler")), "got {names:?}");
    }

    #[test]
    fn prefers_unreal_shipping_binary() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        write_exe(
            &root
                .join("Sable Tide")
                .join("Binaries")
                .join("Win64")
                .join("SableTide-Win64-Shipping.exe"),
            60 * MB,
        );
        write_exe(&root.join("Sable Tide").join("bootstrap.exe"), 200 * 1024);

        let found = scan_root(root, &HashSet::new());
        assert_eq!(found.len(), 1, "small bootstrap should not qualify: {found:?}");
        assert!(found[0].path.ends_with("SableTide-Win64-Shipping.exe"));
        assert!(found[0].reasons.iter().any(|r| r.contains("Unreal")));
    }

    #[test]
    fn marks_known_games() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let exe = root.join("Emberfall").join("Emberfall.exe");
        write_exe(&exe, 30 * MB);

        let canonical = exe.canonicalize().unwrap().to_string_lossy().to_string();
        let known: HashSet<String> = [canonical].into_iter().collect();

        let found = scan_root(root, &known);
        assert_eq!(found.len(), 1);
        assert!(found[0].already_in_library);
    }

    #[test]
    fn scores_are_ordered() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_exe(&root.join("Tiny Tool").join("tool.exe"), 30 * MB);
        write_exe(&root.join("Big Game").join("BigGame.exe"), 80 * MB);

        let found = scan_root(root, &HashSet::new());
        assert!(found.len() >= 2);
        assert!(found[0].score >= found[1].score);
        assert!(found[0].path.ends_with("BigGame.exe"), "got {found:?}");
    }

    #[test]
    fn clean_title_strips_release_noise() {
        assert_eq!(clean_title("Hades_v1.38261.2_(59229)_[FitGirl Repack]"), "Hades");
        assert_eq!(
            clean_title("Ori.and.the.Will.of.the.Wisps-CODEX"),
            "Ori and the Will of the Wisps"
        );
        assert_eq!(clean_title("Sable_Tide"), "Sable Tide");
        assert_eq!(clean_title("Elden Ring"), "Elden Ring");
        assert_eq!(clean_title("Setup_v2.0").is_empty(), false);
    }

    #[test]
    fn clean_title_never_returns_empty_for_real_folders() {
        // Everything stripped: fall back to the raw folder rather than "".
        assert!(!clean_title("v1.0.0").is_empty());
    }

    #[test]
    fn title_folder_skips_build_directories() {
        let cases = [
            (r"C:\Games\Elden Ring\eldenring.exe", "Elden Ring"),
            (r"C:\Games\Elden Ring\Game\eldenring.exe", "Elden Ring"),
            (r"C:\Games\Cyberpunk 2077\bin\x64\Cyberpunk2077.exe", "Cyberpunk 2077"),
            (r"C:\Games\Sable Tide\Binaries\Win64\SableTide-Win64-Shipping.exe", "Sable Tide"),
            (
                r"C:\SteamLibrary\steamapps\common\Elden Ring\Game\eldenring.exe",
                "Elden Ring",
            ),
            (r"D:\Hades\Hades.exe", "Hades"),
        ];

        for (path, expected) in cases {
            assert_eq!(
                title_folder(Path::new(path)).as_deref(),
                Some(expected),
                "for {path}"
            );
        }
    }

    /// The exact shape of the bug a user hit: an executable one level below the
    /// game folder used to produce the title "Game", whose SteamGridDB search
    /// returns a different game entirely.
    #[test]
    fn nested_executable_is_named_after_the_game_not_its_build_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write_exe(&root.join("Elden Ring").join("Game").join("eldenring.exe"), 48 * MB);

        let found = scan_root(root, &HashSet::new());
        assert_eq!(found.len(), 1, "got {found:?}");
        assert_eq!(found[0].name_hint, "Elden Ring");
        assert_ne!(found[0].name_hint, "Game");
    }
}


