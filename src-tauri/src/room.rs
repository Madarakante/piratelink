//! Local rooms: a TCP presence listener, LAN discovery, and ZeroTier helpers.
//!
//! A room is deliberately thin. The host opens a TCP listener that answers
//! `PING` and accepts `HELLO <nickname>`; guests reach it over the local
//! network, or over a ZeroTier address when the players are not on one LAN.
//! Discovery is a UDP broadcast on a fixed port: a guest sends a probe, every
//! discoverable host answers with a small JSON beacon. Nothing in a beacon is
//! trusted beyond what it takes to render a row and build an invite, and
//! nothing is executed.
//!
//! Discovery is unauthenticated by design. It reveals the game title, host
//! address and player count to anyone on the same broadcast domain, which is
//! why the host can turn it off per room.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{log_line, to_string_err};

/// Fixed UDP port every PirateLink instance probes. One listener per machine,
/// which is all a room host needs.
const DISCOVERY_PORT: u16 = 48765;
const PROBE: &[u8] = b"PIRATELINK/PROBE/1\n";
const PROBE_TEXT: &str = "PIRATELINK/PROBE/1";
const ROOM_PREFIX: &str = "PIRATELINK/ROOM/1 ";
const MAX_PACKET: usize = 1024;
const BEACON_VERSION: u8 = 1;

/// The live room owned by this process, if any.
#[derive(Default)]
pub struct RoomState {
    inner: Mutex<Option<RoomRuntime>>,
}

struct RoomRuntime {
    token: String,
    port: u16,
    stop: Arc<AtomicBool>,
    players: Arc<Mutex<Vec<RoomPlayer>>>,
}

#[derive(Clone, Serialize)]
pub struct RoomPlayer {
    nickname: String,
    address: String,
    last_seen: u64,
}

#[derive(Serialize)]
pub struct RoomView {
    token: String,
    port: u16,
    host_address: String,
    network_id: String,
    game: String,
    discoverable: bool,
}

#[derive(Deserialize)]
pub struct StartRoom {
    game: String,
    #[serde(default)]
    network_id: String,
    #[serde(default)]
    host_address: String,
    #[serde(default = "default_discoverable")]
    discoverable: bool,
}

fn default_discoverable() -> bool {
    true
}

#[derive(Serialize)]
pub struct PingResult {
    pub reachable: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct LanStatus {
    pub address: Option<String>,
    pub message: String,
}

#[derive(Serialize)]
pub struct ZeroTierStatus {
    pub network_id: Option<String>,
    pub status: String,
    pub address: Option<String>,
    pub message: String,
}

#[derive(Serialize)]
pub struct DiscoveredRoom {
    token: String,
    game: String,
    host_address: String,
    port: u16,
    network_id: String,
    players: usize,
}

/// Wire format for a discovery reply. `v` gates parsing before any field is
/// trusted; everything here is display data and connection details.
#[derive(Clone, Serialize, Deserialize)]
struct Beacon {
    v: u8,
    token: String,
    game: String,
    host: String,
    port: u16,
    network: String,
    players: usize,
}

impl From<Beacon> for DiscoveredRoom {
    fn from(beacon: Beacon) -> Self {
        Self {
            token: beacon.token,
            game: beacon.game,
            host_address: beacon.host,
            port: beacon.port,
            network_id: beacon.network,
            players: beacon.players,
        }
    }
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or_default()
}

fn room_token() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    format!("{:x}-{:x}", now, std::process::id())
}

fn valid_network_id(value: &str) -> bool {
    value.len() == 16 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn valid_lan_host(value: &str) -> bool {
    match value.parse::<Ipv4Addr>() {
        Ok(ip) => {
            !(ip.is_unspecified()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_multicast())
        }
        Err(_) => false,
    }
}

fn usable_lan_address(ip: Ipv4Addr) -> bool {
    !(ip.is_unspecified() || ip.is_loopback() || ip.is_link_local())
}

/// The address other machines on the LAN should dial.
///
/// Connecting a UDP socket picks the interface the default route uses without
/// sending a packet, which is the same answer `ipconfig` would give for the
/// active adapter but needs no shell parsing.
fn primary_ipv4() -> Option<String> {
    for target in ["8.8.8.8:80", "1.1.1.1:80"] {
        let Ok(socket) = UdpSocket::bind(("0.0.0.0", 0)) else {
            continue;
        };
        if socket.connect(target).is_err() {
            continue;
        }
        let Ok(local) = socket.local_addr() else {
            continue;
        };
        if let IpAddr::V4(ip) = local.ip() {
            if usable_lan_address(ip) {
                return Some(ip.to_string());
            }
        }
    }
    None
}

fn prune_players(players: &mut Vec<RoomPlayer>) {
    let cutoff = now_seconds().saturating_sub(30);
    players.retain(|player| player.last_seen >= cutoff);
}

fn encoded_room(beacon: &Beacon) -> Option<String> {
    let payload = serde_json::to_string(beacon).ok()?;
    let packet = format!("{ROOM_PREFIX}{payload}\n");
    (packet.len() <= MAX_PACKET).then_some(packet)
}

fn decode_room(packet: &[u8]) -> Option<Beacon> {
    if packet.is_empty() || packet.len() > MAX_PACKET {
        return None;
    }
    let text = std::str::from_utf8(packet).ok()?;
    let payload = text.trim_end().strip_prefix(ROOM_PREFIX)?;
    let beacon: Beacon = serde_json::from_str(payload).ok()?;
    if beacon.v != BEACON_VERSION || beacon.token.is_empty() || beacon.token.len() > 64 {
        return None;
    }
    if beacon.port == 0 || beacon.players > 64 {
        return None;
    }
    let host: IpAddr = beacon.host.parse().ok()?;
    if host.is_unspecified() || host.is_multicast() {
        return None;
    }
    Some(beacon)
}

/// Stop and drop the active room. Called before a new room replaces it so the
/// fixed discovery port is released promptly.
fn stop_active(state: &RoomState, reason: &str) {
    let previous = state
        .inner
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take();
    if let Some(active) = previous {
        active.stop.store(true, Ordering::Relaxed);
        log_line(&format!(
            "room: stopped {}:{} ({reason})",
            active.token, active.port
        ));
        // Give the discovery thread one read timeout to close its socket.
        thread::sleep(Duration::from_millis(450));
    }
}

fn spawn_presence(
    listener: TcpListener,
    players: Arc<Mutex<Vec<RoomPlayer>>>,
    stop: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, address)) => {
                    // A peer that connects and never sends a line must not stall
                    // the accept loop; close the socket after a short wait, and
                    // cap the line so a hostile client cannot grow the buffer.
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let mut line = String::new();
                    let read = stream
                        .try_clone()
                        .ok()
                        .map(|clone| {
                            BufReader::new(clone.take(256))
                                .read_line(&mut line)
                                .unwrap_or(0)
                        })
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
                        let mut known = players.lock().unwrap_or_else(|error| error.into_inner());
                        prune_players(&mut known);
                        let entry = RoomPlayer {
                            nickname,
                            address: address.ip().to_string(),
                            last_seen: now_seconds(),
                        };
                        if let Some(existing) =
                            known.iter_mut().find(|player| player.address == entry.address)
                        {
                            *existing = entry;
                        } else {
                            known.push(entry);
                        }
                        let _ = stream.write_all(b"WELCOME\n");
                        log_line(&format!("room: presence from {}", address));
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    log_line(&format!("room: listener stopped: {error}"));
                    break;
                }
            }
        }
    });
}

fn spawn_discovery(
    beacon: Beacon,
    port: u16,
    players: Arc<Mutex<Vec<RoomPlayer>>>,
    stop: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let mut socket = None;
        for attempt in 0..20 {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            match UdpSocket::bind(("0.0.0.0", port)) {
                Ok(bound) => {
                    socket = Some(bound);
                    break;
                }
                Err(error) => {
                    if attempt == 0 {
                        log_line(&format!("room: discovery bind retry: {error}"));
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
        let Some(socket) = socket else {
            log_line("room: LAN discovery unavailable (UDP port busy)");
            return;
        };
        let _ = socket.set_read_timeout(Some(Duration::from_millis(400)));
        let mut buffer = [0_u8; 256];
        while !stop.load(Ordering::Relaxed) {
            match socket.recv_from(&mut buffer) {
                Ok((read, from)) => {
                    if String::from_utf8_lossy(&buffer[..read]).trim() != PROBE_TEXT {
                        continue;
                    }
                    let mut live = beacon.clone();
                    live.players = players
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .len();
                    if let Some(packet) = encoded_room(&live) {
                        let _ = socket.send_to(packet.as_bytes(), from);
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(error) => {
                    log_line(&format!("room: discovery stopped: {error}"));
                    break;
                }
            }
        }
    });
}

#[tauri::command]
pub fn detect_lan() -> LanStatus {
    match primary_ipv4() {
        Some(address) => LanStatus {
            message: format!("This PC is {address} on the local network"),
            address: Some(address),
        },
        None => LanStatus {
            address: None,
            message: "No LAN address found. Enter this PC's IPv4 address manually.".to_string(),
        },
    }
}

#[tauri::command]
pub fn detect_zerotier(network_id: Option<String>) -> Result<ZeroTierStatus, String> {
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
    let wanted = network_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let selected = networks
        .as_array()
        .and_then(|items| {
            items.iter().find(|item| {
                let matches_id = wanted.is_none_or(|id| {
                    item.get("nwid").and_then(|value| value.as_str()) == Some(id)
                });
                matches_id
                    && item.get("status").and_then(|value| value.as_str()) == Some("OK")
            })
        })
        .or_else(|| networks.as_array().and_then(|items| items.first()));

    let Some(network) = selected else {
        return Ok(ZeroTierStatus {
            network_id: None,
            status: "NOT_FOUND".to_string(),
            address: None,
            message: "No ZeroTier networks are joined".to_string(),
        });
    };

    let id = network
        .get("nwid")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let status = network
        .get("status")
        .and_then(|value| value.as_str())
        .unwrap_or("UNKNOWN")
        .to_string();
    // ZeroTier can hand out 10/8, 172.16/12 or 192.168/16; the old check
    // hardcoded 10.* and silently missed the other two ranges.
    let address = network
        .get("assignedAddresses")
        .and_then(|value| value.as_array())
        .and_then(|values| {
            values.iter().find_map(|value| {
                let address = value.as_str()?.split('/').next()?;
                let ip = address.parse::<Ipv4Addr>().ok()?;
                usable_lan_address(ip).then(|| address.to_string())
            })
        });
    let message = if status == "OK" && address.is_some() {
        "ZeroTier network is ready".to_string()
    } else {
        format!("ZeroTier network status: {status}")
    };

    Ok(ZeroTierStatus {
        network_id: id,
        status,
        address,
        message,
    })
}

#[tauri::command]
pub fn join_zerotier(network_id: String) -> Result<String, String> {
    let network_id = network_id.trim();
    if !valid_network_id(network_id) {
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
        Command::new("zerotier-cli")
            .args(["join", network_id])
            .output()
    }
    .map_err(|error| format!("could not run ZeroTier: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(if stderr.is_empty() { stdout } else { stderr });
    }
    Ok(if stdout.is_empty() {
        "ZeroTier join requested".to_string()
    } else {
        stdout
    })
}

#[tauri::command]
pub fn ping_host(host: String, port: u16) -> Result<PingResult, String> {
    let address = format!("{}:{}", host.trim(), port);
    match TcpStream::connect_timeout(
        &address
            .parse()
            .map_err(|error| format!("invalid host address: {error}"))?,
        Duration::from_secs(3),
    ) {
        Ok(mut stream) => {
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
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
pub fn announce_room_player(host: String, port: u16, nickname: String) -> Result<String, String> {
    let nickname = nickname.trim();
    if nickname.is_empty() || nickname.len() > 32 {
        return Err("nickname must contain 1 to 32 characters".to_string());
    }
    let address = format!("{}:{}", host.trim(), port);
    let mut stream = TcpStream::connect_timeout(
        &address
            .parse()
            .map_err(|error| format!("invalid host address: {error}"))?,
        Duration::from_secs(3),
    )
    .map_err(|error| format!("could not reach host: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
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
pub fn room_players(state: tauri::State<'_, RoomState>) -> Result<Vec<RoomPlayer>, String> {
    let room = state
        .inner
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let Some(active) = room.as_ref() else {
        return Ok(Vec::new());
    };
    let mut players = active
        .players
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    prune_players(&mut players);
    Ok(players.clone())
}

#[tauri::command]
pub fn discover_rooms(
    timeout_ms: Option<u64>,
    state: tauri::State<'_, RoomState>,
) -> Result<Vec<DiscoveredRoom>, String> {
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(1200).clamp(300, 3000));
    let own_token = state
        .inner
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .map(|room| room.token.clone());

    let socket = UdpSocket::bind(("0.0.0.0", 0)).map_err(to_string_err)?;
    socket.set_broadcast(true).map_err(to_string_err)?;
    socket
        .set_read_timeout(Some(Duration::from_millis(150)))
        .map_err(to_string_err)?;
    socket
        .send_to(PROBE, format!("255.255.255.255:{DISCOVERY_PORT}"))
        .map_err(|error| format!("could not probe the local network: {error}"))?;

    let deadline = Instant::now() + timeout;
    let mut found: Vec<DiscoveredRoom> = Vec::new();
    let mut buffer = [0_u8; 2048];
    while Instant::now() < deadline {
        match socket.recv_from(&mut buffer) {
            Ok((read, _from)) => {
                let Some(beacon) = decode_room(&buffer[..read]) else {
                    continue;
                };
                if own_token.as_deref() == Some(beacon.token.as_str()) {
                    continue;
                }
                if found.iter().any(|room| room.token == beacon.token) {
                    continue;
                }
                found.push(beacon.into());
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(format!("discovery failed: {error}")),
        }
    }
    found.sort_by_key(|room| room.game.to_lowercase());
    Ok(found)
}

#[tauri::command]
pub fn start_room(request: StartRoom, state: tauri::State<'_, RoomState>) -> Result<RoomView, String> {
    let game = request.game.trim();
    if game.is_empty() || game.chars().count() > 120 {
        return Err("game title must contain 1 to 120 characters".to_string());
    }
    let network = request.network_id.trim();
    if !network.is_empty() && !valid_network_id(network) {
        return Err("ZeroTier network ID must be 16 hexadecimal characters".to_string());
    }
    let host = request.host_address.trim();
    let host = if host.is_empty() {
        primary_ipv4().ok_or_else(|| {
            "Could not detect this PC's LAN address. Enter it manually.".to_string()
        })?
    } else {
        if !valid_lan_host(host) {
            return Err("host address must be a LAN IPv4 address, for example 192.168.1.20".to_string());
        }
        host.to_string()
    };

    // Release the discovery port before the new room claims it.
    stop_active(&state, "replaced");

    let listener = TcpListener::bind("0.0.0.0:0").map_err(to_string_err)?;
    listener.set_nonblocking(true).map_err(to_string_err)?;
    let port = listener.local_addr().map_err(to_string_err)?.port();
    let stop = Arc::new(AtomicBool::new(false));
    let players = Arc::new(Mutex::new(Vec::new()));

    spawn_presence(listener, Arc::clone(&players), Arc::clone(&stop));

    let token = room_token();
    let discoverable = request.discoverable;
    if discoverable {
        spawn_discovery(
            Beacon {
                v: BEACON_VERSION,
                token: token.clone(),
                game: game.to_string(),
                host: host.clone(),
                port,
                network: network.to_string(),
                players: 0,
            },
            DISCOVERY_PORT,
            Arc::clone(&players),
            Arc::clone(&stop),
        );
    }

    let view = RoomView {
        token: token.clone(),
        port,
        host_address: host.clone(),
        network_id: network.to_string(),
        game: game.to_string(),
        discoverable,
    };
    let mut room = state
        .inner
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    *room = Some(RoomRuntime {
        token,
        port,
        stop,
        players,
    });
    log_line(&format!(
        "room: started {game} on {host}:{port} ({})",
        if network.is_empty() {
            "LAN"
        } else {
            "ZeroTier"
        }
    ));
    Ok(view)
}

#[tauri::command]
pub fn stop_room(state: tauri::State<'_, RoomState>) -> Result<(), String> {
    stop_active(&state, "stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beacon() -> Beacon {
        Beacon {
            v: BEACON_VERSION,
            token: "abc-123".to_string(),
            game: "Hades".to_string(),
            host: "192.168.1.20".to_string(),
            port: 39001,
            network: String::new(),
            players: 2,
        }
    }

    #[test]
    fn beacon_round_trips() {
        let packet = encoded_room(&beacon()).expect("encoded");
        let decoded = decode_room(packet.as_bytes()).expect("decoded");
        assert_eq!(decoded.token, "abc-123");
        assert_eq!(decoded.host, "192.168.1.20");
        assert_eq!(decoded.port, 39001);
        assert_eq!(decoded.players, 2);
    }

    #[test]
    fn decode_rejects_garbage_and_other_versions() {
        assert!(decode_room(b"hello").is_none());
        assert!(decode_room(b"PIRATELINK/PROBE/1\n").is_none());

        let mut other = beacon();
        other.v = 9;
        let packet = encoded_room(&other).unwrap();
        assert!(decode_room(packet.as_bytes()).is_none());
    }

    #[test]
    fn decode_rejects_oversized_packets() {
        assert!(decode_room(&vec![b'a'; MAX_PACKET + 1]).is_none());
    }

    #[test]
    fn decode_rejects_bad_connection_details() {
        let mut bad_host = beacon();
        bad_host.host = "not-an-ip".to_string();
        assert!(decode_room(encoded_room(&bad_host).unwrap().as_bytes()).is_none());

        let mut no_token = beacon();
        no_token.token.clear();
        assert!(decode_room(encoded_room(&no_token).unwrap().as_bytes()).is_none());

        let mut no_port = beacon();
        no_port.port = 0;
        assert!(decode_room(encoded_room(&no_port).unwrap().as_bytes()).is_none());
    }

    #[test]
    fn network_ids_are_16_hex_characters() {
        assert!(valid_network_id("743993800f6ed928"));
        assert!(!valid_network_id("743993800f6ed92"));
        assert!(!valid_network_id("743993800f6ed92z"));
    }

    #[test]
    fn lan_hosts_reject_loopback_and_link_local() {
        assert!(valid_lan_host("192.168.1.20"));
        assert!(valid_lan_host("10.39.120.10"));
        assert!(!valid_lan_host("127.0.0.1"));
        assert!(!valid_lan_host("169.254.10.1"));
        assert!(!valid_lan_host("224.0.0.1"));
        assert!(!valid_lan_host("0.0.0.0"));
        assert!(!valid_lan_host("example.com"));
    }

    #[test]
    fn players_expire_after_thirty_seconds() {
        let mut players = vec![
            RoomPlayer {
                nickname: "fresh".into(),
                address: "192.168.1.2".into(),
                last_seen: now_seconds(),
            },
            RoomPlayer {
                nickname: "stale".into(),
                address: "192.168.1.3".into(),
                last_seen: now_seconds().saturating_sub(31),
            },
        ];
        prune_players(&mut players);
        assert_eq!(players.len(), 1);
        assert_eq!(players[0].nickname, "fresh");
    }

    #[test]
    fn presence_listener_answers_ping_and_hello() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("addr").port();
        let stop = Arc::new(AtomicBool::new(false));
        let players = Arc::new(Mutex::new(Vec::new()));
        spawn_presence(listener, Arc::clone(&players), Arc::clone(&stop));

        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("timeout");
        stream.write_all(b"PING\n").expect("write");
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).expect("read");
        assert_eq!(line.trim(), "PONG");

        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("timeout");
        stream.write_all(b"HELLO Guest\n").expect("write");
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).expect("read");
        assert_eq!(line.trim(), "WELCOME");

        let deadline = Instant::now() + Duration::from_secs(2);
        while players.lock().unwrap().is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let known = players.lock().unwrap().clone();
        assert_eq!(known.len(), 1);
        assert_eq!(known[0].nickname, "Guest");

        stop.store(true, Ordering::Relaxed);
    }

    #[test]
    fn discovery_responder_answers_a_probe() {
        let stop = Arc::new(AtomicBool::new(false));
        let players = Arc::new(Mutex::new(vec![RoomPlayer {
            nickname: "Guest".into(),
            address: "192.168.1.9".into(),
            last_seen: now_seconds(),
        }]));
        let mut beacon = beacon();
        beacon.game = "LAN discovery test".to_string();
        spawn_discovery(
            beacon,
            DISCOVERY_PORT,
            Arc::clone(&players),
            Arc::clone(&stop),
        );

        let client = UdpSocket::bind(("127.0.0.1", 0)).expect("client bind");
        client
            .set_read_timeout(Some(Duration::from_millis(800)))
            .expect("timeout");
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut packet = [0_u8; 2048];
        let mut decoded = None;
        while Instant::now() < deadline {
            let _ = client.send_to(PROBE, ("127.0.0.1", DISCOVERY_PORT));
            if let Ok((read, _)) = client.recv_from(&mut packet) {
                if let Some(beacon) = decode_room(&packet[..read]) {
                    decoded = Some(beacon);
                    break;
                }
            }
        }
        stop.store(true, Ordering::Relaxed);

        let beacon = decoded.expect("a discovery reply");
        assert_eq!(beacon.game, "LAN discovery test");
        assert_eq!(beacon.players, 1);
    }
}
