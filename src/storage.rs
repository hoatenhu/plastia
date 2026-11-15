use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize, Serialize)]
struct LockMetadata {
    pid: u32,
    started_at: u64,
}

pub struct WriterLock {
    path: PathBuf,
    _file: File,
}

impl WriterLock {
    pub fn acquire(home: &Path) -> crate::Result<Self> {
        fs::create_dir_all(home.join("memories"))
            .map_err(|_| "could not create memory directory")?;
        let path = home.join("write.lock");
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                let metadata = LockMetadata {
                    pid: std::process::id(),
                    started_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                };
                serde_json::to_writer(&mut file, &metadata)
                    .map_err(|_| "could not write writer lock metadata")?;
                file.flush()
                    .map_err(|_| "could not flush writer lock metadata")?;
                Ok(Self { path, _file: file })
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let data =
                    fs::read_to_string(&path).map_err(|_| "could not read writer lock metadata")?;
                let metadata: LockMetadata = serde_json::from_str(&data)
                    .map_err(|_| format!("invalid writer lock: {}", path.display()))?;
                if process_exists(metadata.pid) {
                    return Err(format!(
                        "store is busy: writer PID {} holds {}",
                        metadata.pid,
                        path.display()
                    )
                    .into());
                }
                fs::remove_file(&path).map_err(|_| "could not remove stale writer lock")?;
                let file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map_err(|error| -> Box<dyn std::error::Error> { Box::new(error) })?;
                let mut lock = Self { path, _file: file };
                let metadata = LockMetadata {
                    pid: std::process::id(),
                    started_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                };
                serde_json::to_writer(&mut lock._file, &metadata)
                    .map_err(|_| "could not write writer lock metadata")?;
                lock._file
                    .flush()
                    .map_err(|_| "could not flush writer lock metadata")?;
                Ok(lock)
            }
            Err(error) => Err(Box::new(error)),
        }
    }
}

impl Drop for WriterLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn process_exists(pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(true)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

pub fn open(home: &Path) -> Result<Connection> {
    std::fs::create_dir_all(home.join("memories"))
        .map_err(|_| rusqlite::Error::InvalidPath(home.to_path_buf()))?;
    let conn = Connection::open(home.join("memory.sqlite3"))?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA busy_timeout=5000;
         PRAGMA foreign_keys=ON;
         CREATE TABLE IF NOT EXISTS events (
           id TEXT PRIMARY KEY,
           scope TEXT NOT NULL,
           source TEXT NOT NULL,
           occurred_at TEXT NOT NULL,
           text TEXT NOT NULL,
           processed INTEGER NOT NULL DEFAULT 0
         );
         CREATE VIRTUAL TABLE IF NOT EXISTS events_fts USING fts5(
           text, content='events', content_rowid='rowid', tokenize='unicode61'
         );
         CREATE TRIGGER IF NOT EXISTS events_ai AFTER INSERT ON events BEGIN
           INSERT INTO events_fts(rowid,text) VALUES (new.rowid,new.text);
         END;
         CREATE TRIGGER IF NOT EXISTS events_ad AFTER DELETE ON events BEGIN
           INSERT INTO events_fts(events_fts,rowid,text) VALUES('delete',old.rowid,old.text);
         END;
         CREATE TABLE IF NOT EXISTS memories (
           id TEXT PRIMARY KEY,
           scope TEXT NOT NULL,
           kind TEXT NOT NULL,
           memory_key TEXT NOT NULL,
           text TEXT NOT NULL,
           source_event TEXT NOT NULL,
           status TEXT NOT NULL DEFAULT 'active',
           file_path TEXT NOT NULL,
           created_at TEXT NOT NULL,
           FOREIGN KEY(source_event) REFERENCES events(id)
         );
         CREATE INDEX IF NOT EXISTS memories_lookup ON memories(scope,kind,memory_key,status);
         CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
           text, content='memories', content_rowid='rowid', tokenize='unicode61'
         );
         CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
           INSERT INTO memories_fts(rowid,text) VALUES(new.rowid,new.text);
         END;
         CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
           INSERT INTO memories_fts(memories_fts,rowid,text) VALUES('delete',old.rowid,old.text);
         END;
         CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE OF text ON memories BEGIN
           INSERT INTO memories_fts(memories_fts,rowid,text) VALUES('delete',old.rowid,old.text);
           INSERT INTO memories_fts(rowid,text) VALUES(new.rowid,new.text);
         END;
          CREATE TABLE IF NOT EXISTS conflicts (
            id TEXT PRIMARY KEY,
            scope TEXT NOT NULL,
            memory_key TEXT NOT NULL,
            existing_memory TEXT NOT NULL,
            incoming_event TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending'
          );
          CREATE TABLE IF NOT EXISTS outbox (
            id TEXT PRIMARY KEY,
            event_id TEXT NOT NULL,
            scope TEXT NOT NULL,
            kind TEXT NOT NULL,
            memory_key TEXT NOT NULL,
            memory_id TEXT NOT NULL,
            text TEXT NOT NULL,
            markdown TEXT NOT NULL,
            file_path TEXT NOT NULL,
            created_at TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            FOREIGN KEY(event_id) REFERENCES events(id)
          );
          CREATE INDEX IF NOT EXISTS outbox_pending ON outbox(status);
          CREATE INDEX IF NOT EXISTS outbox_event ON outbox(event_id);
          CREATE INDEX IF NOT EXISTS outbox_memory ON outbox(memory_id)
          ;",
    )?;
    for statement in [
        "ALTER TABLE memories ADD COLUMN supersedes TEXT",
        "ALTER TABLE memories ADD COLUMN valid_from TEXT",
        "ALTER TABLE memories ADD COLUMN valid_until TEXT",
        "ALTER TABLE memories ADD COLUMN evidence_type TEXT NOT NULL DEFAULT 'observed'",
        "ALTER TABLE conflicts ADD COLUMN resolution TEXT NOT NULL DEFAULT 'pending'",
        "ALTER TABLE conflicts ADD COLUMN resolved_at TEXT",
        "ALTER TABLE conflicts ADD COLUMN resolution_reason TEXT",
    ] {
        if let Err(error) = conn.execute(statement, []) {
            if !error.to_string().contains("duplicate column name") {
                return Err(error);
            }
        }
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS audit_log (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           action TEXT NOT NULL,
           conflict_id TEXT,
           memory_id TEXT,
           details TEXT NOT NULL,
           created_at TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS audit_log_conflict ON audit_log(conflict_id);",
    )?;
    Ok(conn)
}
