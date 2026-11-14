use rusqlite::{Connection, Result};
use std::path::Path;

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
    Ok(conn)
}
