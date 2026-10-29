mod artwork;
mod config;
mod db;
mod emulator;
mod scanner;
mod sgdb;

use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use serde::{Deserialize, Serialize};
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
    /// Behind a lock so the settings page can change the emulator path at
    /// runtime; every reader takes a short lock and clones what it needs.
    config: std::sync::RwLock<Config>,
    sgdb: Sgdb,
    room: Mutex<Option<RoomRuntime>>,
}

struct RoomRuntime {
    token: String,
    port: u16,
    stop: Arc<AtomicBool>,
    players: Arc<Mutex<Vec<RoomPlayer>>>,
}

#[derive(Clone, Serialize)]
struct RoomPlayer {
    nickname: String,
    address: String,
    last_seen: u64,
}

#[derive(Serialize)]
struct PingResult {
    reachable: bool,
    message: String,
}

#[derive(Serialize)]
struct RoomView {
    token: String,
    port: u16,
    host_address: String,
    network_id: String,
    game: String,
}

#[derive(Deserialize)]
struct StartRoom {
    game: String,
    network_id: String,
    host_address: String,
}

#[derive(Serialize)]
struct ZeroTierStatus {
    network_id: Option<String>,
    status: String,
    address: Option<String>,
    message: String,
}

impl AppState {
    fn config(&self) -> std::sync::RwLockReadGuard<'_, Config> {
        self.config.read().unwrap_or_else(|err| err.into_inner())
    }
}

fn to_string_err<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

fn room_token() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    format!("{:x}-{:x}", now, std::process::id())
}

#[tauri::command]
fn detect_zerotier(network_id: Option<String>) -> Result<ZeroTierStatus, String> {
    let cli = [
        PathBuf::from(r"C:\Program Files (x86)\ZeroTier\One\zerotier-cli.bat"),
        PathBuf::from(r"C:\Program Files\ZeroTier\One\zerotier-cli.bat"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .unwrap_or_else(|| PathBuf::from("zerotier-cli"));
    let output = Command::new(&cli)
        .args(["-j", "listnetworks"])
        .output()
        .map_err(|error| format!("could not run {}: {error}", cli.display()))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let networks: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("could not read ZeroTier response: {error}"))?;
    let wanted = network_id.as_deref().map(str::trim).filter(|value| !value.is_empty());
    let selected = networks
        .as_array()
        .and_then(|items| items.iter().find(|item| {
            let matches_id = wanted.is_none_or(|id| item.get("nwid").and_then(|value| value.as_str()) == Some(id));
            matches_id && item.get("status").and_then(|value| value.as_str()) == Some("OK")
        }))
        .or_else(|| networks.as_array().and_then(|items| items.first()));

    let Some(network) = selected else {
        return Ok(ZeroTierStatus {
            network_id: None,
            status: "NOT_FOUND".to_string(),
            address: None,
            message: "No ZeroTier networks are joined".to_string(),
        });
    };

    let id = network.get("nwid").and_then(|value| value.as_str()).map(str::to_string);
    let status = network
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("UNKNOWN")
        .to_string();
    let address = network
        .get("assignedAddresses")
        .and_then(|value| value.as_array())
        .and_then(|values| values.iter().find_map(|value| {
            let address = value.as_str()?.split('/').next()?;
            address.starts_with("10.").then(|| address.to_string())
        }));
    let message = if status == "OK" && address.is_some() {
        "ZeroTier network is ready".to_string()
    } else {
        format!("ZeroTier network status: {status}")
    };

    Ok(ZeroTierStatus { network_id: id, status, address, message })
}

#[tauri::command]
fn join_zerotier(network_id: String) -> Result<String, String> {
    let network_id = network_id.trim();
    if !network_id.chars().all(|ch| ch.is_ascii_hexdigit()) || network_id.len() != 16 {
        return Err("ZeroTier network ID must be 16 hexadecimal characters".to_string());
    }

    let cli = [
        PathBuf::from(r"C:\Program Files (x86)\ZeroTier\One\zerotier-cli.bat"),
        PathBuf::from(r"C:\Program Files\ZeroTier\One\zerotier-cli.bat"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .unwrap_or_else(|| PathBuf::from("zerotier-cli.bat"));

    let output = if cli.is_file() {
        Command::new("cmd")
            .args(["/C", cli.to_string_lossy().as_ref(), "join", network_id])
            .output()
    } else {
        Command::new("zerotier-cli").args(["join", network_id]).output()
    }
    .map_err(|error| format!("could not run ZeroTier: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(if stderr.is_empty() { stdout } else { stderr });
    }
    Ok(if stdout.is_empty() { "ZeroTier join requested".to_string() } else { stdout })
}

fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default()
}

fn prune_players(players: &mut Vec<RoomPlayer>) {
    let cutoff = now_seconds().saturating_sub(30);
    players.retain(|player| player.last_seen >= cutoff);
}

#[tauri::command]
fn ping_host(host: String, port: u16) -> Result<PingResult, String> {
    let address = format!("{}:{}", host.trim(), port);
    match TcpStream::connect_timeout(
        &address.parse().map_err(|error| format!("invalid host address: {error}"))?,
        std::time::Duration::from_secs(3),
    ) {
        Ok(mut stream) => {
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .map_err(to_string_err)?;
            stream.write_all(b"PING\n").map_err(to_string_err)?;
            let mut response = [0_u8; 16];
            let read = stream.read(&mut response).map_err(to_string_err)?;
            let reachable = &response[..read] == b"PONG\n";
            Ok(PingResult {
                reachable,
                message: if reachable {
                    format!("Host reachable at {address}")
                } else {
                    "Host answered unexpectedly".to_string()
                },
            })
        }
        Err(error) => Ok(PingResult {
            reachable: false,
            message: format!("Host unreachable at {address}: {error}"),
        }),
    }
}

#[tauri::command]
fn announce_room_player(host: String, port: u16, nickname: String) -> Result<String, String> {
    let nickname = nickname.trim();
    if nickname.is_empty() || nickname.len() > 32 {
        return Err("nickname must contain 1 to 32 characters".to_string());
    }
    let address = format!("{}:{}", host.trim(), port);
    let mut stream = TcpStream::connect_timeout(
        &address.parse().map_err(|error| format!("invalid host address: {error}"))?,
        std::time::Duration::from_secs(3),
    )
    .map_err(|error| format!("could not reach host: {error}"))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .map_err(to_string_err)?;
    stream
        .write_all(format!("HELLO {}\n", nickname.replace(['\r', '\n'], " ")).as_bytes())
        .map_err(to_string_err)?;
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .map_err(to_string_err)?;
    if response.trim() == "WELCOME" {
        Ok("Connected to room host".to_string())
    } else {
        Err(format!("host rejected presence: {}", response.trim()))
    }
}

#[tauri::command]
fn room_players(state: tauri::State<'_, AppState>) -> Result<Vec<RoomPlayer>, String> {
    let room = state.room.lock().unwrap_or_else(|error| error.into_inner());
    let Some(active) = room.as_ref() else { return Ok(Vec::new()); };
    let mut players = active.players.lock().unwrap_or_else(|error| error.into_inner());
    prune_players(&mut players);
    Ok(players.clone())
}

#[tauri::command]
fn start_room(request: StartRoom, state: tauri::State<'_, AppState>) -> Result<RoomView, String> {
    let game = request.game.trim();
    let network_id = request.network_id.trim();
    let host_address = request.host_address.trim();
    if game.is_empty() || network_id.is_empty() || host_address.is_empty() {
        return Err("game, ZeroTier network ID, and host address are required".to_string());
    }
    if !network_id.chars().all(|ch| ch.is_ascii_hexdigit()) || network_id.len() != 16 {
        return Err("ZeroTier network ID must be 16 hexadecimal characters".to_string());
    }

    let listener = TcpListener::bind("0.0.0.0:0").map_err(to_string_err)?;
    listener.set_nonblocking(true).map_err(to_string_err)?;
    let port = listener.local_addr().map_err(to_string_err)?.port();
    let stop = Arc::new(AtomicBool::new(false));
    let players = Arc::new(Mutex::new(Vec::new()));
    let stop_thread = Arc::clone(&stop);
    let players_thread = Arc::clone(&players);
    thread::spawn(move || {
        while !stop_thread.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, address)) => {
                    let mut line = String::new();
                    let read = stream
                        .try_clone()
                        .ok()
                        .map(|clone| BufReader::new(clone).read_line(&mut line).unwrap_or(0))
                        .unwrap_or(0);
                    if read == 0 {
                        continue;
                    }
                    if line.trim() == "PING" {
                        let _ = stream.write_all(b"PONG\n");
                        continue;
                    }
                    if let Some(nickname) = line.strip_prefix("HELLO ") {
                        let nickname = nickname.trim().to_string();
                        if nickname.is_empty() || nickname.len() > 32 {
                            let _ = stream.write_all(b"REJECTED\n");
                            continue;
                        }
                        let mut known = players_thread.lock().unwrap_or_else(|error| error.into_inner());
                        prune_players(&mut known);
                        let entry = RoomPlayer {
                            nickname,
                            address: address.ip().to_string(),
                            last_seen: now_seconds(),
                        };
                        if let Some(existing) = known.iter_mut().find(|player| player.address == entry.address) {
                            *existing = entry;
                        } else {
                            known.push(entry);
                        }
                        let _ = stream.write_all(b"WELCOME\n");
                        log_line(&format!("room: presence from {}", address));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(std::time::Duration::from_millis(100));
                }
                Err(error) => {
                    log_line(&format!("room: listener stopped: {error}"));
                    break;
                }
            }
        }
    });

    let token = room_token();
    let view = RoomView {
        token: token.clone(),
        port,
        host_address: host_address.to_string(),
        network_id: network_id.to_string(),
        game: game.to_string(),
    };
    let mut room = state.room.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(previous) = room.replace(RoomRuntime { token, port, stop, players }) {
        previous.stop.store(true, Ordering::Relaxed);
    }
    log_line(&format!("room: started {} on {}:{}", game, host_address, port));
    Ok(view)
}

#[tauri::command]
fn stop_room(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut room = state.room.lock().unwrap_or_else(|error| error.into_inner());
    if let Some(active) = room.take() {
        active.stop.store(true, Ordering::Relaxed);
        log_line(&format!("room: stopped {}:{}", active.token, active.port));
    }
    Ok(())
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
    let file = Path::new(&path);
    let install_dir = file
        .parent()
        .ok_or_else(|| "game file has no parent directory".to_string())?
        .to_string_lossy()
        .to_string();

    // Derived here, never taken from the webview: the extension is the only thing
    // that decides whether a row is started directly or handed to an emulator.
    let platform = platform_for(file).to_string();
    log_line(&format!("add_game: {path} ({platform}) -> {title:?}"));

    state
        .db
        .add_game(&NewGame {
            title,
            executable: path,
            install_dir,
            platform,
        })
        .map_err(to_string_err)
}

/// `"switch"` for a console ROM, `"pc"` for a Windows executable.
fn platform_for(path: &Path) -> &'static str {
    if scanner::is_rom(path) {
        "switch"
    } else {
        "pc"
    }
}

#[tauri::command]
fn remove_game(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.remove_game(id).map_err(to_string_err)
}

/// What a chosen file looks like before it joins the library.
#[derive(Serialize)]
struct GameFileInfo {
    path: String,
    title: String,
    install_dir: String,
    /// `"pc"` or `"switch"`.
    platform: String,
    size_bytes: u64,
    already_in_library: bool,
}

/// Inspects one game file picked by the user: a Windows executable or a
/// console ROM.
///
/// The suggested title for an executable comes from the install folder rather
/// than the filename, because a filename is frequently not the game's name
/// (`eldenring.exe`, `hl2.exe`, `Cyberpunk2077.exe`). A ROM is the opposite: the
/// dump is named after the game, so the filename is cleaned of ids and versions
/// instead. Either way the caller shows the suggestion in an editable field, so
/// a wrong guess costs a keystroke, not a wrong cover.
#[tauri::command]
fn inspect_file(path: String, state: tauri::State<'_, AppState>) -> Result<GameFileInfo, String> {
    let file = Path::new(&path);
    let meta = std::fs::metadata(file)
        .map_err(|err| format!("cannot read {}: {err}", file.display()))?;

    if !meta.is_file() {
        log_line(&format!("inspect_file: not a file: {}", file.display()));
        return Err(format!("not a file: {}", file.display()));
    }

    let is_exe = file
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
    if !is_exe && !scanner::is_rom(file) {
        log_line(&format!(
            "inspect_file: unsupported extension: {}",
            file.display()
        ));
        return Err(format!(
            "not a game file: {} (expected .exe, .nsp, .xci or .nro)",
            file.display()
        ));
    }

    let canonical = file
        .canonicalize()
        .unwrap_or_else(|_| file.to_path_buf())
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

    let platform = platform_for(file);
    let title = if platform == "switch" {
        scanner::title_for_rom(file)
    } else {
        scanner::title_for_exe(file)
    };
    log_line(&format!(
        "inspect_file: {} ({platform}) -> {title:?}",
        file.display()
    ));

    Ok(GameFileInfo {
        path: file.to_string_lossy().to_string(),
        title,
        install_dir: file
            .parent()
            .map(|parent| parent.to_string_lossy().to_string())
            .unwrap_or_default(),
        platform: platform.to_string(),
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
            "no confident SteamGridDB match for \"{}\" ({} result{} searched) - artwork was left unchanged",
            game.title,
            matches.len(),
            if matches.len() == 1 { "" } else { "s" }
        )
    })?;

    let art_dir = state.config().art_dir();
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

/// One candidate in the manual picker's search results.
#[derive(Serialize)]
struct ArtGame {
    id: i64,
    name: String,
    verified: bool,
}

/// One thumbnail in the manual picker.
#[derive(Serialize)]
struct ArtOption {
    id: i64,
    thumb: String,
    /// Full-size asset, used when the choice is applied and for a larger preview.
    url: String,
    width: u32,
    height: u32,
}

#[derive(Serialize)]
struct ArtChoices {
    covers: Vec<ArtOption>,
    heroes: Vec<ArtOption>,
}

/// Free-text SteamGridDB search for the manual picker.
///
/// Unlike the import path this makes no similarity judgement: the user is looking
/// at the names and picking one. That is exactly what unblocks titles SteamGridDB
/// spells differently from the folder they came in (FC 26 is listed as
/// "EA SPORTS FCT 26"), which the automatic matcher must refuse.
#[tauri::command]
async fn search_artwork(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ArtGame>, String> {
    if !state.sgdb.has_key() {
        return Err("SteamGridDB API key not configured".to_string());
    }
    let term = query.trim();
    if term.len() < 2 {
        return Ok(Vec::new());
    }

    let results = state.sgdb.search(term).await.map_err(to_string_err)?;
    Ok(results
        .into_iter()
        .take(24)
        .map(|game| ArtGame {
            id: game.id,
            name: game.name,
            verified: game.verified,
        })
        .collect())
}

/// Every usable asset SteamGridDB has for one game, as thumbnails.
///
/// Tagged artwork is filtered here for the same reason it is filtered in
/// `pick_best`: offering it would make the picker a way around the veto.
#[tauri::command]
async fn artwork_choices(
    sgdb_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<ArtChoices, String> {
    if !state.sgdb.has_key() {
        return Err("SteamGridDB API key not configured".to_string());
    }

    let option = |asset: &sgdb::Asset| ArtOption {
        id: asset.id,
        thumb: asset.thumb.clone(),
        url: asset.url.clone(),
        width: asset.width,
        height: asset.height,
    };

    let covers = state
        .sgdb
        .grids(sgdb_id, None)
        .await
        .map_err(to_string_err)?
        .iter()
        .filter(|asset| sgdb::is_safe(asset))
        .take(24)
        .map(option)
        .collect();

    let heroes = state
        .sgdb
        .heroes(sgdb_id)
        .await
        .map_err(to_string_err)?
        .iter()
        .filter(|asset| sgdb::is_safe(asset))
        .take(12)
        .map(option)
        .collect();

    Ok(ArtChoices { covers, heroes })
}

/// Applies artwork the user picked by hand.
///
/// Either asset id may be absent, in which case that slot keeps whatever it had:
/// changing a cover should not silently discard a hero the user chose earlier.
/// The chosen SteamGridDB id is recorded even when nothing is replaced, so a later
/// refresh starts from the right entry instead of re-guessing the title.
#[tauri::command]
async fn apply_artwork(
    id: i64,
    sgdb_id: i64,
    cover_id: Option<i64>,
    hero_id: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<Artwork, String> {
    state
        .db
        .game(id)
        .map_err(to_string_err)?
        .ok_or_else(|| format!("no game with id {id}"))?;

    if !state.sgdb.has_key() {
        return Err("SteamGridDB API key not configured".to_string());
    }

    let art_dir = state.config().art_dir();
    std::fs::create_dir_all(&art_dir).map_err(to_string_err)?;

    let mut artwork = Artwork {
        sgdb_id: Some(sgdb_id),
        cover: None,
        hero: None,
    };

    if let Some(want) = cover_id {
        let grids = state.sgdb.grids(sgdb_id, None).await.map_err(to_string_err)?;
        let asset = grids
            .iter()
            .find(|asset| asset.id == want)
            .ok_or_else(|| format!("cover {want} is no longer offered by SteamGridDB"))?;
        let bytes = state.sgdb.fetch_bytes(&asset.url).await.map_err(to_string_err)?;
        let file = artwork::store(
            &bytes,
            &art_dir,
            &format!("{id}-cover"),
            artwork::COVER_MAX_WIDTH,
        )?;
        artwork.cover = Some(file.to_string_lossy().to_string());
    }

    if let Some(want) = hero_id {
        let heroes = state.sgdb.heroes(sgdb_id).await.map_err(to_string_err)?;
        let asset = heroes
            .iter()
            .find(|asset| asset.id == want)
            .ok_or_else(|| format!("hero {want} is no longer offered by SteamGridDB"))?;
        let bytes = state.sgdb.fetch_bytes(&asset.url).await.map_err(to_string_err)?;
        let file = artwork::store(
            &bytes,
            &art_dir,
            &format!("{id}-hero"),
            artwork::HERO_MAX_WIDTH,
        )?;
        artwork.hero = Some(file.to_string_lossy().to_string());
    }

    log_line(&format!(
        "apply_artwork: {id} <- sgdb {sgdb_id} cover {:?} hero {:?}",
        cover_id, hero_id
    ));

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
/// for game *id* N, but it cannot ask for an arbitrary executable. A Windows game
/// is started directly; a console ROM is handed to the emulator found for it.
#[tauri::command]
fn launch_game(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let game = state
        .db
        .game(id)
        .map_err(to_string_err)?
        .ok_or_else(|| format!("no game with id {id}"))?;

    let file = Path::new(&game.executable).to_path_buf();
    if !file.is_file() {
        return Err(format!("game file not found: {}", game.executable));
    }

    if game.platform == "switch" {
        // Clone the configured path so the lock is not held while the emulator
        // starts (and so a poisoned lock cannot take the launch down).
        let configured = state.config().ryujinx.clone();
        return launch_rom(&game, &file, configured.as_deref());
    }

    let dir = file
        .parent()
        .ok_or_else(|| "executable has no parent directory".to_string())?;

    // Many games resolve assets relative to their working directory, so set it to
    // the install folder rather than inheriting the launcher's.
    Command::new(&file)
        .current_dir(dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not start {}: {e}", file.display()))
}

/// Opens a ROM with the Switch emulator, or explains exactly what is missing.
fn launch_rom(
    game: &Game,
    rom: &Path,
    configured: Option<&Path>,
) -> Result<(), String> {
    let emulator = emulator::find(configured, Some(rom)).ok_or_else(|| {
        log_line(&format!("launch_game: no emulator for {}", rom.display()));
        format!(
            "no Switch emulator found for \"{}\" - put Ryujinx on PATH, install it in a \
             standard location, or name its executable in config.json as \"ryujinx_path\"",
            game.title
        )
    })?;

    log_line(&format!(
        "launch_game: {} -> {} {}",
        game.title,
        emulator.display(),
        rom.display()
    ));

    emulator::launch(&emulator, rom)
        .map(|_| ())
        .map_err(|e| format!("could not start {}: {e}", emulator.display()))
}

/// Where a Switch game would be opened from, so the UI can say it plainly.
#[derive(Serialize)]
struct EmulatorStatus {
    path: Option<String>,
}

#[tauri::command]
fn emulator_status(state: tauri::State<'_, AppState>) -> EmulatorStatus {
    let configured = state.config().ryujinx.clone();
    let found = emulator::find(configured.as_deref(), None);
    EmulatorStatus {
        path: found.map(|path| path.to_string_lossy().to_string()),
    }
}

// -- settings ---------------------------------------------------------------

/// What the settings page shows.
///
/// Paths are absolute so the sheet can print them verbatim. The API key is never
/// returned: only whether one exists and its last four characters, which is
/// enough to recognise the key without moving it into the webview.
#[derive(Serialize)]
struct SettingsView {
    api_key_set: bool,
    api_key_tail: String,
    /// The path written in config.json, empty when the emulator is auto-detected.
    ryujinx_path: String,
    /// What auto-detection actually found, whatever the source.
    ryujinx_found: Option<String>,
    hero_fit: String,
    glyphs: String,
    sound: bool,
    data_dir: String,
    art_dir: String,
    database: String,
    log_path: String,
    config_path: String,
    games: usize,
    art_files: usize,
    art_bytes: u64,
    version: String,
}

#[derive(Deserialize)]
struct SettingsPatch {
    /// Absent means "leave it alone"; an empty string clears the key.
    steamgriddb_api_key: Option<String>,
    /// Empty string means "find the emulator yourself".
    ryujinx_path: Option<String>,
    hero_fit: Option<String>,
    glyphs: Option<String>,
    sound: Option<bool>,
}

fn count_art(dir: &Path) -> (usize, u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (0, 0);
    };
    entries
        .flatten()
        .filter_map(|entry| entry.metadata().ok())
        .filter(|meta| meta.is_file())
        .fold((0, 0), |(files, bytes), meta| (files + 1, bytes + meta.len()))
}

fn settings_view(state: &AppState) -> SettingsView {
    let config = state.config();
    let art_dir = config.art_dir();
    let (art_files, art_bytes) = count_art(&art_dir);
    let games = state.db.games().map(|games| games.len()).unwrap_or(0);
    let configured = config.ryujinx.clone();
    let found = emulator::find(configured.as_deref(), None);

    let key = config.api_key.trim();
    SettingsView {
        api_key_set: !key.is_empty(),
        // Last four characters only: enough to recognise the key, never enough to
        // use it. Taken by chars so a pasted non-ASCII character cannot panic, and
        // only for keys long enough that four characters are not most of the key.
        api_key_tail: if key.chars().count() >= 8 {
            key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect()
        } else {
            String::new()
        },
        ryujinx_path: configured
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default(),
        ryujinx_found: found.map(|path| path.to_string_lossy().to_string()),
        hero_fit: config.hero_fit.clone(),
        glyphs: config.glyphs.clone(),
        sound: config.sound,
        data_dir: config.data_dir.to_string_lossy().to_string(),
        art_dir: art_dir.to_string_lossy().to_string(),
        database: db::db_path(&config.data_dir).to_string_lossy().to_string(),
        log_path: config.log_path().to_string_lossy().to_string(),
        config_path: config.config_path().to_string_lossy().to_string(),
        games,
        art_files,
        art_bytes,
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> SettingsView {
    settings_view(&state)
}

/// Applies the patch, writes config.json, and returns the result.
///
/// Values are applied in memory even if the write fails, so the app behaves the
/// way the controls say; the error is still returned so the sheet can say the
/// file was not updated.
#[tauri::command]
fn save_settings(
    patch: SettingsPatch,
    state: tauri::State<'_, AppState>,
) -> Result<SettingsView, String> {
    {
        let mut config = state
            .config
            .write()
            .unwrap_or_else(|err| err.into_inner());

        if let Some(key) = patch.steamgriddb_api_key.as_deref() {
            config.set_api_key(key);
            state.sgdb.set_key(&config.api_key);
        }
        if let Some(path) = patch.ryujinx_path.as_deref() {
            config.set_ryujinx(path);
        }
        if let Some(value) = patch.hero_fit.as_deref() {
            config.set_hero_fit(value);
        }
        if let Some(value) = patch.glyphs.as_deref() {
            config.set_glyphs(value);
        }
        if let Some(on) = patch.sound {
            config.set_sound(on);
        }

        log_line(&format!(
            "save_settings: hero_fit={} glyphs={} sound={} key={} emulator={}",
            config.hero_fit,
            config.glyphs,
            config.sound,
            if config.api_key.is_empty() {
                "cleared"
            } else {
                "set"
            },
            config
                .ryujinx
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "auto".to_string())
        ));

        config
            .save()
            .map_err(|err| format!("could not write config.json: {err}"))?;
    }

    Ok(settings_view(&state))
}

/// Opens one of the app's own locations in Explorer. The path is chosen here, not
/// passed from the webview, so this cannot be used to open arbitrary folders.
#[tauri::command]
fn reveal_data(kind: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let (path, is_file) = {
        let config = state.config();
        match kind.as_str() {
            "data" => (config.data_dir.clone(), false),
            "art" => (config.art_dir(), false),
            "log" => (config.log_path(), true),
            "config" => (config.config_path(), true),
            other => return Err(format!("unknown location {other:?}")),
        }
    };

    if !is_file {
        std::fs::create_dir_all(&path).map_err(to_string_err)?;
    }

    // A file that no longer exists would make Explorer fall back to Documents, so
    // reveal its folder instead.
    let target = if is_file && !path.is_file() {
        path.parent().map(Path::to_path_buf).unwrap_or(path)
    } else {
        path
    };

    let mut command = Command::new("explorer");
    if is_file && target.is_file() {
        // `/select,` highlights the file instead of opening it.
        command.arg(format!("/select,{}", target.display()));
    } else {
        command.arg(&target);
    }

    command
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("could not open Explorer: {err}"))
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

            app.manage(AppState {
                db,
                config: std::sync::RwLock::new(config),
                sgdb,
                room: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            library_roots,
            add_library_root,
            remove_library_root,
            library_list,
            scan_library,
            add_game,
            inspect_file,
            remove_game,
            sgdb_status,
            fetch_artwork,
            search_artwork,
            artwork_choices,
            apply_artwork,
            launch_game,
            emulator_status,
            get_settings,
            save_settings,
            reveal_data,
            detect_zerotier,
            join_zerotier,
            ping_host,
            announce_room_player,
            room_players,
            start_room,
            stop_room,
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
                platform: "pc".into(),
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
