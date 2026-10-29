mod artwork;
mod config;
mod db;
mod scanner;
mod sgdb;

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use serde::Serialize;
use tauri::Manager;

use config::Config;
use db::{Db, Game, NewGame, Root};
use scanner::Candidate;
use sgdb::{pick_best, Sgdb};

/// Append-only command log under the app data directory.
///
/// The release build has no console (`windows_subsystem = "windows"`), so a
/// command that fails or stalls leaves nothing behind. Every command writes an
/// entry and an exit here, which is the difference between "it hangs" and a
/// timestamped trace.
static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

fn log_line(message: &str) {
    let Some(path) = LOG_PATH.get() else { return };
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "[{secs}] {message}");
    }
}

struct AppState {
    db: Db,
    config: Config,
    sgdb: Sgdb,
}

fn to_string_err<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

// -- library ---------------------------------------------------------------

#[tauri::command]
fn library_roots(state: tauri::State<'_, AppState>) -> Result<Vec<Root>, String> {
    state.db.roots().map_err(to_string_err)
}

#[tauri::command]
fn add_library_root(path: String, state: tauri::State<'_, AppState>) -> Result<Root, String> {
    log_line(&format!("add_library_root: {path}"));
    state
        .db
        .add_root(Path::new(&path))
        .map_err(to_string_err)
}

#[tauri::command]
fn remove_library_root(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.remove_root(id).map_err(to_string_err)
}

#[tauri::command]
fn library_list(state: tauri::State<'_, AppState>) -> Result<Vec<Game>, String> {
    state.db.games().map_err(to_string_err)
}

/// Walks every configured root. Disk I/O runs on the blocking pool so the webview
/// thread never stalls on a slow drive.
#[tauri::command]
async fn scan_library(state: tauri::State<'_, AppState>) -> Result<Vec<Candidate>, String> {
    let started = std::time::Instant::now();
    log_line("scan_library: start");

    let roots: Vec<PathBuf> = state
        .db
        .roots()
        .map_err(to_string_err)?
        .into_iter()
        .map(|r| PathBuf::from(r.path))
        .collect();

    let known: HashSet<String> = state
        .db
        .games()
        .map_err(to_string_err)?
        .into_iter()
        .map(|g| {
            Path::new(&g.executable)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(&g.executable))
                .to_string_lossy()
                .to_string()
        })
        .collect();

    log_line(&format!(
        "scan_library: {} root(s), {} already known",
        roots.len(),
        known.len()
    ));

    let joined = tauri::async_runtime::spawn_blocking(move || {
        let mut all = Vec::new();
        for root in roots {
            let found = scanner::scan_root(&root, &known);
            log_line(&format!("scan_library: {} -> {} candidate(s)", root.display(), found.len()));
            all.extend(found);
        }
        all.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| b.size_bytes.cmp(&a.size_bytes)));
        all
    })
    .await;

    match joined {
        Ok(all) => {
            log_line(&format!(
                "scan_library: done in {} ms, {} candidate(s)",
                started.elapsed().as_millis(),
                all.len()
            ));
            Ok(all)
        }
        Err(err) => {
            log_line(&format!("scan_library: task failed: {err}"));
            Err(err.to_string())
        }
    }
}

#[tauri::command]
fn add_game(path: String, title: String, state: tauri::State<'_, AppState>) -> Result<Game, String> {
    let exe = Path::new(&path);
    let install_dir = exe
        .parent()
        .ok_or_else(|| "executable has no parent directory".to_string())?
        .to_string_lossy()
        .to_string();

    state
        .db
        .add_game(&NewGame {
            title,
            executable: path,
            install_dir,
        })
        .map_err(to_string_err)
}

#[tauri::command]
fn remove_game(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.remove_game(id).map_err(to_string_err)
}

/// What a chosen .exe looks like before it joins the library.
#[derive(Serialize)]
struct ExecutableInfo {
    path: String,
    title: String,
    install_dir: String,
    size_bytes: u64,
    already_in_library: bool,
}

/// Inspects one executable picked by the user.
///
/// The suggested title comes from the install folder rather than the filename,
/// because a filename is frequently not the game's name (`eldenring.exe`,
/// `hl2.exe`, `Cyberpunk2077.exe`). The caller shows that suggestion in an
/// editable field, so a wrong guess costs a keystroke, not a wrong cover.
#[tauri::command]
fn inspect_executable(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<ExecutableInfo, String> {
    let exe = Path::new(&path);
    let meta = std::fs::metadata(exe)
        .map_err(|err| format!("cannot read {}: {err}", exe.display()))?;

    if !meta.is_file() {
        log_line(&format!("inspect_executable: not a file: {}", exe.display()));
        return Err(format!("not a file: {}", exe.display()));
    }
    if !exe
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        log_line(&format!("inspect_executable: not an .exe: {}", exe.display()));
        return Err(format!("not an .exe: {}", exe.display()));
    }

    let canonical = exe
        .canonicalize()
        .unwrap_or_else(|_| exe.to_path_buf())
        .to_string_lossy()
        .to_string();

    let already_in_library = state
        .db
        .games()
        .map_err(to_string_err)?
        .iter()
        .any(|game| {
            Path::new(&game.executable)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(&game.executable))
                .to_string_lossy()
                == canonical
        });

    let title = scanner::title_for_exe(exe);
    log_line(&format!(
        "inspect_executable: {} -> {title:?}",
        exe.display()
    ));

    Ok(ExecutableInfo {
        path: exe.to_string_lossy().to_string(),
        title,
        install_dir: exe
            .parent()
            .map(|parent| parent.to_string_lossy().to_string())
            .unwrap_or_default(),
        size_bytes: meta.len(),
        already_in_library,
    })
}

// -- artwork ---------------------------------------------------------------

#[derive(Serialize)]
struct SgdbStatus {
    configured: bool,
}

#[derive(Serialize)]
struct Artwork {
    sgdb_id: Option<i64>,
    cover: Option<String>,
    hero: Option<String>,
}

#[tauri::command]
fn sgdb_status(state: tauri::State<'_, AppState>) -> SgdbStatus {
    SgdbStatus {
        configured: state.sgdb.has_key(),
    }
}

/// Resolves a library entry against SteamGridDB and caches the chosen art.
///
/// Failures are non-fatal for the caller: a game without artwork still belongs in
/// the library, so this returns the artwork it managed to store.
#[tauri::command]
async fn fetch_artwork(id: i64, state: tauri::State<'_, AppState>) -> Result<Artwork, String> {
    let game = state
        .db
        .game(id)
        .map_err(to_string_err)?
        .ok_or_else(|| format!("no game with id {id}"))?;

    if !state.sgdb.has_key() {
        return Err("SteamGridDB API key not configured".to_string());
    }

    let matches = state.sgdb.search(&game.title).await.map_err(to_string_err)?;

    // Only proceed on a match that is actually close to the library title. A wrong
    // cover is silent and gets cached, which is worse than leaving the game without
    // artwork and telling the user why.
    let best = sgdb::best_match(&game.title, &matches).ok_or_else(|| {
        format!(
            "no confident SteamGridDB match for \"{}\" ({} result{} searched) — artwork was left unchanged",
            game.title,
            matches.len(),
            if matches.len() == 1 { "" } else { "s" }
        )
    })?;

    let art_dir = state.config.art_dir();
    std::fs::create_dir_all(&art_dir).map_err(to_string_err)?;

    let mut artwork = Artwork {
        sgdb_id: Some(best.id),
        cover: None,
        hero: None,
    };

    if let Some(asset) = pick_best(&state.sgdb.grids(best.id, None).await.map_err(to_string_err)?) {
        let bytes = state.sgdb.fetch_bytes(&asset.url).await.map_err(to_string_err)?;
        let file = artwork::store(
            &bytes,
            &art_dir,
            &format!("{id}-cover"),
            artwork::COVER_MAX_WIDTH,
        )?;
        artwork.cover = Some(file.to_string_lossy().to_string());
    }

    if let Some(asset) = pick_best(&state.sgdb.heroes(best.id).await.map_err(to_string_err)?) {
        let bytes = state.sgdb.fetch_bytes(&asset.url).await.map_err(to_string_err)?;
        let file = artwork::store(
            &bytes,
            &art_dir,
            &format!("{id}-hero"),
            artwork::HERO_MAX_WIDTH,
        )?;
        artwork.hero = Some(file.to_string_lossy().to_string());
    }

    state
        .db
        .set_artwork(
            id,
            artwork.sgdb_id,
            artwork.cover.as_deref(),
            artwork.hero.as_deref(),
            None,
        )
        .map_err(to_string_err)?;

    Ok(artwork)
}

// -- launching -------------------------------------------------------------

/// Launches a library entry.
///
/// The path comes from the database, never from the frontend: the webview can ask
/// for game *id* N, but it cannot ask for an arbitrary executable.
#[tauri::command]
fn launch_game(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let game = state
        .db
        .game(id)
        .map_err(to_string_err)?
        .ok_or_else(|| format!("no game with id {id}"))?;

    let exe = Path::new(&game.executable).to_path_buf();
    if !exe.is_file() {
        return Err(format!("executable not found: {}", game.executable));
    }

    let dir = exe
        .parent()
        .ok_or_else(|| "executable has no parent directory".to_string())?;

    // Many games resolve assets relative to their working directory, so set it to
    // the install folder rather than inheriting the launcher's.
    Command::new(&exe)
        .current_dir(dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not start {}: {e}", exe.display()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;

            // Route panics into the log before anything else can fail.
            let _ = LOG_PATH.set(data_dir.join("piratelink.log"));
            std::panic::set_hook(Box::new(|info| {
                log_line(&format!("PANIC: {info}"));
            }));

            log_line(&format!(
                "startup: pid {} version {}",
                std::process::id(),
                env!("CARGO_PKG_VERSION")
            ));

            let config = Config::load(data_dir);
            log_line(&format!(
                "startup: data dir {} (api key {})",
                config.data_dir.display(),
                if config.api_key.is_empty() { "missing" } else { "present" }
            ));

            let db = Db::open(&db::db_path(&config.data_dir)).map_err(|e| e.to_string())?;
            log_line("startup: database open");

            let sgdb = Sgdb::new(config.api_key.clone());

            app.manage(AppState { db, config, sgdb });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            library_roots,
            add_library_root,
            remove_library_root,
            library_list,
            scan_library,
            add_game,
            inspect_executable,
            remove_game,
            sgdb_status,
            fetch_artwork,
            launch_game,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PirateLink");
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    use std::collections::HashSet;
    use std::fs::File;

    /// The Phase 2 acceptance test, minus the Tauri plumbing: a fake install is
    /// scanned, added to a real library, resolved against the live API, and the
    /// chosen cover is written to the cache and recorded.
    ///
    ///     $env:PIRATELINK_SGDB_KEY = "..."
    ///     cargo test -- --ignored live_pipeline
    #[test]
    #[ignore = "network: needs PIRATELINK_SGDB_KEY"]
    fn live_pipeline_scan_add_and_cache_artwork() {
        let key = std::env::var("PIRATELINK_SGDB_KEY").expect("PIRATELINK_SGDB_KEY");
        let work = tempfile::tempdir().unwrap();

        // A release-style folder that the scanner has to see through.
        let install = work.path().join("Hades");
        std::fs::create_dir_all(&install).unwrap();
        let exe = install.join("Hades.exe");
        File::create(&exe).unwrap().set_len(40 * 1024 * 1024).unwrap();

        let candidates = scanner::scan_root(work.path(), &HashSet::new());
        assert_eq!(candidates.len(), 1, "expected one candidate: {candidates:?}");
        let candidate = &candidates[0];
        assert!(candidate.path.ends_with("Hades.exe"));
        assert_eq!(candidate.name_hint, "Hades");

        let db = Db::open_in_memory().unwrap();
        let game = db
            .add_game(&NewGame {
                title: candidate.name_hint.clone(),
                executable: candidate.path.clone(),
                install_dir: candidate.install_dir.clone(),
            })
            .unwrap();

        let client = Sgdb::new(key);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let (bytes, sgdb_id) = rt.block_on(async {
            let matches = client.search(&game.title).await.unwrap();
            let best = matches
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(&game.title))
                .expect("exact match");
            let grids = client.grids(best.id, None).await.unwrap();
            let asset = pick_best(&grids).expect("a cover");
            (client.fetch_bytes(&asset.url).await.unwrap(), best.id)
        });

        let art_dir = work.path().join("cache").join("art");
        std::fs::create_dir_all(&art_dir).unwrap();
        let cover = art_dir.join(format!("{}-cover.png", game.id));
        std::fs::write(&cover, &bytes).unwrap();

        db.set_artwork(game.id, Some(sgdb_id), Some(cover.to_str().unwrap()), None, None)
            .unwrap();

        let stored = db.game(game.id).unwrap().unwrap();
        assert_eq!(stored.sgdb_id, Some(4997889), "Hades is a stable id");
        assert!(bytes.len() > 10_000, "cover looks truncated: {}", bytes.len());
        assert!(cover.is_file());
    }
}
