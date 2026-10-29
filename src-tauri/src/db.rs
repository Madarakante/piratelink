//! SQLite-backed library.
//!
//! One file under the app data dir, no ORM, migrations gated on `PRAGMA user_version`.
//! Every method takes `&self` and locks internally: the Tauri commands are async and
//! the connection is not `Sync`, so the mutex is the boundary.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, DbError>;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("{0}")]
    Invalid(String),
}

/// Bump when the schema changes; the migration block is applied in order.
const SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Root {
    pub id: i64,
    pub path: String,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: i64,
    pub title: String,
    pub executable: String,
    pub install_dir: String,
    /// `"pc"` for a Windows executable, `"switch"` for a ROM an emulator opens.
    pub platform: String,
    pub sgdb_id: Option<i64>,
    pub cover_path: Option<String>,
    pub hero_path: Option<String>,
    pub logo_path: Option<String>,
    pub accent: Option<String>,
    pub playtime_minutes: i64,
    pub last_played_at: Option<String>,
    pub added_at: String,
}

#[derive(Debug, Clone)]
pub struct NewGame {
    pub title: String,
    pub executable: String,
    pub install_dir: String,
    pub platform: String,
}

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| DbError::Invalid(e.to_string()))?;
        }
        let conn = Connection::open(path)?;
        Self::from_connection(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self> {
        // WAL keeps the scanner's writes from blocking reads while the UI is open.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.lock();
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;

        if version < 1 {
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS library_roots (
                    id       INTEGER PRIMARY KEY AUTOINCREMENT,
                    path     TEXT NOT NULL UNIQUE,
                    added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
                );

                CREATE TABLE IF NOT EXISTS games (
                    id                INTEGER PRIMARY KEY AUTOINCREMENT,
                    title             TEXT NOT NULL,
                    sort_title        TEXT NOT NULL,
                    executable        TEXT NOT NULL UNIQUE,
                    install_dir       TEXT NOT NULL,
                    sgdb_id           INTEGER,
                    cover_path        TEXT,
                    hero_path         TEXT,
                    logo_path         TEXT,
                    accent            TEXT,
                    playtime_minutes  INTEGER NOT NULL DEFAULT 0,
                    last_played_at    TEXT,
                    added_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
                );

                CREATE INDEX IF NOT EXISTS idx_games_sort ON games(sort_title);
                "#,
            )?;
        }

        if version < 2 {
            // A row has to say which kind of file `executable` points at, because
            // a Switch ROM is handed to an emulator instead of being started.
            // Every row that already exists is a Windows executable.
            conn.execute_batch("ALTER TABLE games ADD COLUMN platform TEXT NOT NULL DEFAULT 'pc';")?;
        }

        if version < SCHEMA_VERSION {
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }

        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        // A poisoned mutex means a previous caller panicked mid-write; the data is
        // still readable, so recover instead of propagating the panic.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    // -- roots ------------------------------------------------------------

    pub fn roots(&self) -> Result<Vec<Root>> {
        let conn = self.lock();
        let mut stmt = conn.prepare("SELECT id, path, added_at FROM library_roots ORDER BY path")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Root {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    added_at: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Adds a root if it is an existing directory. Idempotent.
    pub fn add_root(&self, path: &Path) -> Result<Root> {
        if !path.is_dir() {
            return Err(DbError::Invalid(format!(
                "not a directory: {}",
                path.display()
            )));
        }
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let as_string = canonical.to_string_lossy().to_string();

        let conn = self.lock();
        conn.execute(
            "INSERT OR IGNORE INTO library_roots (path) VALUES (?1)",
            params![as_string],
        )?;
        conn.query_row(
            "SELECT id, path, added_at FROM library_roots WHERE path = ?1",
            params![as_string],
            |row| {
                Ok(Root {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    added_at: row.get(2)?,
                })
            },
        )
        .map_err(Into::into)
    }

    pub fn remove_root(&self, id: i64) -> Result<()> {
        let conn = self.lock();
        conn.execute("DELETE FROM library_roots WHERE id = ?1", params![id])?;
        Ok(())
    }

    // -- games ------------------------------------------------------------

    pub fn games(&self) -> Result<Vec<Game>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT id, title, executable, install_dir, platform, sgdb_id, cover_path, hero_path,
                    logo_path, accent, playtime_minutes, last_played_at, added_at
             FROM games ORDER BY sort_title, title",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Game {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    executable: row.get(2)?,
                    install_dir: row.get(3)?,
                    platform: row.get(4)?,
                    sgdb_id: row.get(5)?,
                    cover_path: row.get(6)?,
                    hero_path: row.get(7)?,
                    logo_path: row.get(8)?,
                    accent: row.get(9)?,
                    playtime_minutes: row.get(10)?,
                    last_played_at: row.get(11)?,
                    added_at: row.get(12)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn game(&self, id: i64) -> Result<Option<Game>> {
        Ok(self.games()?.into_iter().find(|g| g.id == id))
    }

    pub fn add_game(&self, new: &NewGame) -> Result<Game> {
        if !Path::new(&new.executable).is_file() {
            return Err(DbError::Invalid(format!(
                "executable not found: {}",
                new.executable
            )));
        }

        let conn = self.lock();
        conn.execute(
            "INSERT INTO games (title, sort_title, executable, install_dir, platform)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(executable) DO UPDATE SET title = excluded.title, sort_title = excluded.sort_title",
            params![
                new.title,
                sort_key(&new.title),
                new.executable,
                new.install_dir,
                new.platform
            ],
        )?;

        conn.query_row(
            "SELECT id, title, executable, install_dir, platform, sgdb_id, cover_path, hero_path,
                    logo_path, accent, playtime_minutes, last_played_at, added_at
             FROM games WHERE executable = ?1",
            params![new.executable],
            |row| {
                Ok(Game {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    executable: row.get(2)?,
                    install_dir: row.get(3)?,
                    platform: row.get(4)?,
                    sgdb_id: row.get(5)?,
                    cover_path: row.get(6)?,
                    hero_path: row.get(7)?,
                    logo_path: row.get(8)?,
                    accent: row.get(9)?,
                    playtime_minutes: row.get(10)?,
                    last_played_at: row.get(11)?,
                    added_at: row.get(12)?,
                })
            },
        )
        .map_err(Into::into)
    }

    pub fn set_artwork(
        &self,
        id: i64,
        sgdb_id: Option<i64>,
        cover: Option<&str>,
        hero: Option<&str>,
        logo: Option<&str>,
    ) -> Result<()> {
        let conn = self.lock();
        conn.execute(
            "UPDATE games SET sgdb_id = COALESCE(?2, sgdb_id),
                              cover_path = COALESCE(?3, cover_path),
                              hero_path  = COALESCE(?4, hero_path),
                              logo_path  = COALESCE(?5, logo_path)
             WHERE id = ?1",
            params![id, sgdb_id, cover, hero, logo],
        )?;
        Ok(())
    }

    /// Used by the test suite today; wired to real sessions once the launcher can
    /// observe process exit.
    #[allow(dead_code)]
    pub fn record_playtime(&self, id: i64, minutes: i64) -> Result<()> {        let conn = self.lock();
        conn.execute(
            "UPDATE games SET playtime_minutes = playtime_minutes + ?2,
                              last_played_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
             WHERE id = ?1",
            params![id, minutes.max(0)],
        )?;
        Ok(())
    }

    /// Removes a library entry. Artwork on disk is left alone: the cache is
    /// disposable and a later scan may re-add the same game.
    pub fn remove_game(&self, id: i64) -> Result<()> {
        let conn = self.lock();
        conn.execute("DELETE FROM games WHERE id = ?1", params![id])?;
        Ok(())
    }
}

/// Sort key that ignores leading articles and punctuation, so "The Ascent" files
/// under A.
pub fn sort_key(title: &str) -> String {
    let lowered = title.to_lowercase();
    let stripped = ["the ", "a ", "an "]
        .iter()
        .find_map(|prefix| lowered.strip_prefix(prefix))
        .unwrap_or(&lowered);
    stripped
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
}

pub fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join("library.db")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Db {
        Db::open_in_memory().expect("in-memory db")
    }

    fn fake_exe(dir: &Path, name: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, b"MZ").unwrap();
        path
    }

    #[test]
    fn migration_is_idempotent() {
        let db = db();
        db.migrate().unwrap();
        assert!(db.games().unwrap().is_empty());
    }

    #[test]
    fn migration_adds_platform_to_an_existing_library() {
        // A version-1 database has no `platform` column. Opening it must keep the
        // rows and read them as Windows games rather than failing.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                r#"
                CREATE TABLE games (
                    id               INTEGER PRIMARY KEY AUTOINCREMENT,
                    title            TEXT NOT NULL,
                    sort_title       TEXT NOT NULL,
                    executable       TEXT NOT NULL UNIQUE,
                    install_dir      TEXT NOT NULL,
                    sgdb_id          INTEGER,
                    cover_path       TEXT,
                    hero_path        TEXT,
                    logo_path        TEXT,
                    accent           TEXT,
                    playtime_minutes INTEGER NOT NULL DEFAULT 0,
                    last_played_at   TEXT,
                    added_at         TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
                );
                INSERT INTO games (title, sort_title, executable, install_dir)
                VALUES ('Hades', 'hades', 'C:/games/hades.exe', 'C:/games');
                PRAGMA user_version = 1;
                "#,
            )
            .unwrap();
        }

        let db = Db::open(&path).unwrap();
        let games = db.games().unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].platform, "pc", "existing rows are Windows games");
    }

    #[test]
    fn switch_roms_keep_their_platform() {
        let db = db();
        let dir = tempfile::tempdir().unwrap();
        let rom = fake_exe(dir.path(), "Mario Kart 8 Deluxe [0100152000022000][v0].nsp");

        let game = db
            .add_game(&NewGame {
                title: "Mario Kart 8 Deluxe".into(),
                executable: rom.to_string_lossy().to_string(),
                install_dir: dir.path().to_string_lossy().to_string(),
                platform: "switch".into(),
            })
            .unwrap();

        assert_eq!(game.platform, "switch");
        assert_eq!(db.game(game.id).unwrap().unwrap().platform, "switch");
    }

    #[test]
    fn roots_round_trip_and_dedupe() {
        let db = db();
        let dir = tempfile::tempdir().unwrap();
        let a = db.add_root(dir.path()).unwrap();
        let b = db.add_root(dir.path()).unwrap();
        assert_eq!(a.id, b.id, "adding the same root twice must not duplicate");
        assert_eq!(db.roots().unwrap().len(), 1);

        db.remove_root(a.id).unwrap();
        assert!(db.roots().unwrap().is_empty());
    }

    #[test]
    fn rejects_missing_directory() {
        let db = db();
        let err = db.add_root(Path::new("C:/definitely/not/here")).unwrap_err();
        assert!(matches!(err, DbError::Invalid(_)));
    }

    #[test]
    fn add_game_rejects_missing_executable() {
        let db = db();
        let err = db
            .add_game(&NewGame {
                title: "Nope".into(),
                executable: "C:/nope/game.exe".into(),
                install_dir: "C:/nope".into(),
                platform: "pc".into(),
            })
            .unwrap_err();
        assert!(matches!(err, DbError::Invalid(_)));
    }

    #[test]
    fn add_game_is_idempotent_on_executable() {
        let db = db();
        let dir = tempfile::tempdir().unwrap();
        let exe = fake_exe(dir.path(), "game.exe");

        let new = NewGame {
            title: "Hades".into(),
            executable: exe.to_string_lossy().to_string(),
            install_dir: dir.path().to_string_lossy().to_string(),
            platform: "pc".into(),
        };
        let first = db.add_game(&new).unwrap();
        let second = db.add_game(&new).unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(db.games().unwrap().len(), 1);
    }

    #[test]
    fn artwork_is_partial_update() {
        let db = db();
        let dir = tempfile::tempdir().unwrap();
        let exe = fake_exe(dir.path(), "game.exe");
        let game = db
            .add_game(&NewGame {
                title: "Sable Tide".into(),
                executable: exe.to_string_lossy().to_string(),
                install_dir: dir.path().to_string_lossy().to_string(),
                platform: "pc".into(),
            })
            .unwrap();

        db.set_artwork(game.id, Some(42), Some("c.webp"), Some("h.webp"), None)
            .unwrap();
        let stored = db.game(game.id).unwrap().unwrap();
        assert_eq!(stored.sgdb_id, Some(42));
        assert_eq!(stored.cover_path.as_deref(), Some("c.webp"));

        // Cover only: sgdb_id and hero must survive.
        db.set_artwork(game.id, None, Some("c2.webp"), None, None).unwrap();
        let stored = db.game(game.id).unwrap().unwrap();
        assert_eq!(stored.sgdb_id, Some(42));
        assert_eq!(stored.cover_path.as_deref(), Some("c2.webp"));
        assert_eq!(stored.hero_path.as_deref(), Some("h.webp"));
    }

    #[test]
    fn playtime_accumulates() {
        let db = db();
        let dir = tempfile::tempdir().unwrap();
        let exe = fake_exe(dir.path(), "game.exe");
        let game = db
            .add_game(&NewGame {
                title: "Emberfall".into(),
                executable: exe.to_string_lossy().to_string(),
                install_dir: dir.path().to_string_lossy().to_string(),
                platform: "pc".into(),
            })
            .unwrap();

        db.record_playtime(game.id, 30).unwrap();
        db.record_playtime(game.id, 15).unwrap();
        let stored = db.game(game.id).unwrap().unwrap();
        assert_eq!(stored.playtime_minutes, 45);
        assert!(stored.last_played_at.is_some());
    }

    #[test]
    fn sort_key_ignores_articles_and_punctuation() {
        assert_eq!(sort_key("The Ascent"), "ascent");
        assert_eq!(sort_key("A Plague Tale"), "plaguetale");
        assert_eq!(sort_key("Anodyne"), "anodyne");
        assert_eq!(sort_key("S.T.A.L.K.E.R."), "stalker");
    }
}
