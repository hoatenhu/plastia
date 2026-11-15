use crate::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::HashMap,
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Deserialize)]
struct Event {
    #[serde(default)]
    id: Option<String>,
    #[serde(default = "default_scope")]
    scope: String,
    #[serde(default = "default_source")]
    source: String,
    #[serde(default)]
    occurred_at: Option<String>,
    text: String,
}
fn default_scope() -> String {
    "global".to_string()
}
fn default_source() -> String {
    "unknown".to_string()
}
fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
fn digest(s: &str) -> String {
    blake3::hash(s.as_bytes()).to_hex()[..20].to_string()
}

fn outbox_id(event_id: &str, scope: &str, candidate: &Candidate) -> String {
    digest(&format!(
        "outbox\0{}\0{}\0{}\0{}\0{}",
        event_id, scope, candidate.kind, candidate.key, candidate.text
    ))
}

fn failpoint(name: &str) {
    if std::env::var("PLASTIA_FAILPOINT").ok().as_deref() == Some(name) {
        std::process::exit(86);
    }
}

pub fn ingest_file(conn: &mut Connection, file: &Path) -> Result<usize> {
    let contents = fs::read_to_string(file)?;
    let val: serde_json::Value = serde_json::from_str(&contents)?;
    let vals: Vec<serde_json::Value> = match val {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(mut o) if o.contains_key("events") => {
            serde_json::from_value(o.remove("events").unwrap())?
        }
        one @ serde_json::Value::Object(_) => vec![one],
        _ => return Err("expected event object, array or {events: [...]}".into()),
    };
    let mut count = 0;
    let tx = conn.transaction()?;
    for val in vals {
        let e: Event = serde_json::from_value(val)?;
        if e.text.trim().is_empty() || e.scope.trim().is_empty() {
            return Err("event text/scope must not be empty".into());
        }
        let time = e.occurred_at.unwrap_or_else(now);
        let id = e
            .id
            .unwrap_or_else(|| digest(&format!("{}\0{}\0{}\0{}", e.scope, e.source, time, e.text)));
        count += tx.execute(
            "INSERT OR IGNORE INTO events(id,scope,source,occurred_at,text) VALUES (?1,?2,?3,?4,?5)",
            params![id, e.scope, e.source, time, e.text],
        )?;
    }
    tx.commit()?;
    Ok(count)
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub events_reviewed: usize,
    pub memories_created: usize,
    pub duplicates: usize,
    pub conflicts: usize,
    pub dry_run: bool,
}

#[derive(Clone)]
struct Candidate {
    kind: String,
    key: String,
    text: String,
}

// Conservative, deterministic extraction. Other conversation text remains searchable as episodes.
// Explicit durable markers intentionally take precedence over unreliable heuristic guesses.
fn extract(text: &str) -> Vec<Candidate> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches(['-', '*', ' ']).trim();
        for (prefix, kind) in [
            ("remember:", "fact"),
            ("decision:", "decision"),
            ("policy:", "policy"),
            ("lesson:", "lesson"),
            ("preference:", "preference"),
        ] {
            if !line
                .get(..prefix.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
            {
                continue;
            }
            let body = line[prefix.len()..].trim();
            if body.is_empty() {
                break;
            }
            // A stable key is optional, but recommended: 'policy: release = CEO approves'.
            // Bare notes are keyed by full normalized text, so they cannot silently supersede.
            let (key, value) = if let Some((k, v)) = body.split_once('=') {
                if !k.trim().is_empty() && !v.trim().is_empty() {
                    (k.trim().to_lowercase(), v.trim())
                } else {
                    (body.to_lowercase(), body)
                }
            } else {
                (body.to_lowercase(), body)
            };
            out.push(Candidate {
                kind: kind.into(),
                key,
                text: value.into(),
            });
            break;
        }
    }
    out
}

fn encode_markdown(id: &str, scope: &str, c: &Candidate, event: &str, timestamp: &str) -> String {
    // All metadata is JSON-encoded on a single line to avoid YAML/frontmatter injection.
    let meta = json!({"id":id,"scope":scope,"kind":c.kind,"key":c.key,"source_event":event,"created_at":timestamp});
    format!("<!-- plastia:{} -->\n\n# {}\n\n{}\n", meta, c.kind, c.text)
}

fn atomic_write(path: &Path, data: &str) -> Result<()> {
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&temp, data)?;
    fs::rename(&temp, path)?;
    Ok(())
}

pub fn recover(conn: &mut Connection, home: &Path) -> Result<usize> {
    let mut intents = Vec::new();
    {
        let mut query = conn.prepare(
            "SELECT id,event_id,scope,kind,memory_key,memory_id,text,markdown,file_path,created_at FROM outbox WHERE status='pending' ORDER BY rowid",
        )?;
        let rows = query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?;
        for row in rows {
            intents.push(row?);
        }
    }

    let mut applied = 0;
    for (intent_id, event_id, scope, kind, key, memory_id, text, markdown, file_path, created_at) in
        intents
    {
        let path = Path::new(&file_path);
        if path.exists() {
            let existing = fs::read_to_string(path)?;
            if existing != markdown {
                return Err(format!("outbox target differs: {}", path.display()).into());
            }
        } else {
            if path.parent() != Some(home.join("memories").as_path()) {
                return Err(
                    format!("outbox target outside memory directory: {}", path.display()).into(),
                );
            }
            atomic_write(path, &markdown)?;
        }

        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO memories(id,scope,kind,memory_key,text,source_event,file_path,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![memory_id, scope, kind, key, text, event_id, file_path, created_at],
        )?;
        tx.execute(
            "UPDATE outbox SET status='applied' WHERE id=?1",
            [&intent_id],
        )?;
        tx.execute(
            "UPDATE events SET processed=1 WHERE id=?1 AND NOT EXISTS (SELECT 1 FROM outbox WHERE event_id=?1 AND status='pending')",
            [&event_id],
        )?;
        tx.commit()?;
        applied += 1;
    }
    applied += recover_conflict_outbox(conn, home)?;
    Ok(applied)
}

fn recover_conflict_outbox(conn: &mut Connection, home: &Path) -> Result<usize> {
    let mut intents = Vec::new();
    {
        let mut query = conn.prepare("SELECT id,scope,kind,memory_key,memory_id,supersedes,text,source_event,markdown,file_path,created_at FROM conflict_outbox WHERE status='pending' ORDER BY rowid")?;
        let rows = query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
            ))
        })?;
        for row in rows {
            intents.push(row?);
        }
    }
    for (
        intent_id,
        scope,
        kind,
        key,
        memory_id,
        supersedes,
        text,
        event_id,
        markdown,
        file_path,
        created_at,
    ) in &intents
    {
        let path = Path::new(file_path);
        if path.exists() {
            if fs::read_to_string(path)? != *markdown {
                return Err(format!("conflict outbox target differs: {}", path.display()).into());
            }
        } else {
            if path.parent() != Some(home.join("memories").as_path()) {
                return Err(format!(
                    "conflict outbox target outside memory directory: {}",
                    path.display()
                )
                .into());
            }
            atomic_write(path, markdown)?;
        }
        failpoint("after-conflict-file");
        let tx = conn.transaction()?;
        tx.execute("UPDATE memories SET status='superseded', valid_until=?1 WHERE id=?2 AND status='active'", params![created_at, supersedes])?;
        tx.execute("INSERT OR IGNORE INTO memories(id,scope,kind,memory_key,text,source_event,status,supersedes,valid_from,evidence_type,file_path,created_at) VALUES (?1,?2,?3,?4,?5,?6,'active',?7,?8,'observed',?9,?8)", params![memory_id, scope, kind, key, text, event_id, supersedes, created_at, file_path])?;
        tx.execute(
            "UPDATE conflict_outbox SET status='applied' WHERE id=?1",
            [intent_id],
        )?;
        tx.commit()?;
    }
    Ok(intents.len())
}

pub fn consolidate(conn: &mut Connection, home: &Path, dry_run: bool) -> Result<Summary> {
    let mut events = Vec::new();
    {
        let mut q = conn.prepare(
            "SELECT id,scope,occurred_at,text FROM events WHERE processed=0 ORDER BY rowid",
        )?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            events.push(row?);
        }
    }
    let mut s = Summary {
        events_reviewed: 0,
        memories_created: 0,
        duplicates: 0,
        conflicts: 0,
        dry_run,
    };
    for (event_id, scope, timestamp, text) in events {
        s.events_reviewed += 1;
        for c in extract(&text) {
            let existing: Option<(String, String)> = conn.query_row(
                "SELECT id,text FROM memories WHERE scope=?1 AND kind=?2 AND memory_key=?3 AND status='active' LIMIT 1",
                params![scope, c.kind, c.key], |r| Ok((r.get(0)?,r.get(1)?)),
            ).optional()?;
            match existing {
                Some((_, ref old)) if old == &c.text => {
                    s.duplicates += 1;
                }
                Some((old_id, _)) => {
                    s.conflicts += 1;
                    if !dry_run {
                        let id = digest(&format!("conflict\0{}\0{}\0{}", event_id, old_id, c.key));
                        conn.execute("INSERT OR IGNORE INTO conflicts(id,scope,memory_key,existing_memory,incoming_event) VALUES (?1,?2,?3,?4,?5)",
                            params![id, scope, c.key, old_id, event_id])?;
                    }
                }
                None => {
                    s.memories_created += 1;
                    if !dry_run {
                        let id = digest(&format!(
                            "memory\0{}\0{}\0{}\0{}",
                            scope, c.kind, c.key, c.text
                        ));
                        let path = home.join("memories").join(format!("{id}.md"));
                        let md = encode_markdown(&id, &scope, &c, &event_id, &timestamp);
                        let intent_id = outbox_id(&event_id, &scope, &c);
                        {
                            let tx = conn.transaction()?;
                            tx.execute(
                                "INSERT OR IGNORE INTO outbox(id,event_id,scope,kind,memory_key,memory_id,text,markdown,file_path,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                                params![intent_id, event_id, scope, c.kind, c.key, id, c.text, md, path.to_string_lossy().to_string(), timestamp],
                            )?;
                            tx.commit()?;
                        }
                        failpoint("after-outbox");
                        atomic_write(&path, &md)?;
                        conn.execute("INSERT OR IGNORE INTO memories(id,scope,kind,memory_key,text,source_event,file_path,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                            params![id, scope, c.kind, c.key, c.text, event_id, path.to_string_lossy().to_string(), timestamp])?;
                        conn.execute(
                            "UPDATE outbox SET status='applied' WHERE id=?1",
                            [&intent_id],
                        )?;
                    }
                }
            }
        }
        if !dry_run {
            let pending: i64 = conn.query_row(
                "SELECT COUNT(*) FROM outbox WHERE event_id=?1 AND status='pending'",
                [&event_id],
                |row| row.get(0),
            )?;
            if pending == 0 {
                conn.execute("UPDATE events SET processed=1 WHERE id=?1", [&event_id])?;
            }
        }
    }
    Ok(s)
}

#[derive(Debug, Serialize)]
pub struct Hit {
    pub id: String,
    pub kind: String,
    pub scope: String,
    pub text: String,
}

#[derive(Debug)]
pub struct Conflict {
    pub id: String,
    pub scope: String,
    pub memory_key: String,
}

pub fn list_conflicts(conn: &Connection) -> Result<Vec<Conflict>> {
    let mut query = conn.prepare(
        "SELECT id,scope,memory_key FROM conflicts WHERE resolution='pending' ORDER BY rowid",
    )?;
    let rows = query.query_map([], |row| {
        Ok(Conflict {
            id: row.get(0)?,
            scope: row.get(1)?,
            memory_key: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn resolve_conflict(
    conn: &mut Connection,
    home: &Path,
    conflict_id: &str,
    accept: bool,
    reason: &str,
) -> Result<()> {
    if conflict_id.trim().is_empty() || reason.trim().is_empty() {
        return Err("conflict ID and reason must not be empty".into());
    }
    let conflict: (String, String, String, String, String, String) = conn.query_row(
        "SELECT c.scope,c.memory_key,c.existing_memory,c.incoming_event,c.resolution,m.kind FROM conflicts c JOIN memories m ON m.id=c.existing_memory WHERE c.id=?1",
        [conflict_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
    )?;
    if conflict.4 != "pending" {
        return Ok(());
    }
    if accept && conflict.5 == "policy" {
        return Err("policy conflicts require authenticated authority".into());
    }
    let incoming: (String, String, String) = conn.query_row(
        "SELECT id,text,occurred_at FROM events WHERE id=?1",
        [&conflict.3],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let tx = conn.transaction()?;
    if accept {
        let memory_id = digest(&format!(
            "resolved\0{}\0{}\0{}",
            conflict_id, incoming.0, incoming.1
        ));
        let candidate = Candidate {
            kind: conflict.5.clone(),
            key: conflict.1.clone(),
            text: incoming.1.clone(),
        };
        let markdown = encode_markdown(
            &memory_id,
            &conflict.0,
            &candidate,
            &incoming.0,
            &incoming.2,
        );
        let path = home.join("memories").join(format!("{memory_id}.md"));
        if path.exists() && fs::read_to_string(&path)? != markdown {
            return Err(format!("conflict target differs: {}", path.display()).into());
        }
        tx.execute(
            "INSERT OR IGNORE INTO conflict_outbox(id,conflict_id,scope,kind,memory_key,memory_id,supersedes,text,source_event,markdown,file_path,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![conflict_id, conflict_id, conflict.0, conflict.5, conflict.1, memory_id, conflict.2, incoming.1, incoming.0, markdown, path.to_string_lossy().to_string(), incoming.2],
        )?;
    }
    let resolution = if accept { "accepted" } else { "rejected" };
    tx.execute(
        "UPDATE conflicts SET resolution=?1,resolved_at=?2,resolution_reason=?3 WHERE id=?4 AND resolution='pending'",
        params![resolution, now(), reason, conflict_id],
    )?;
    let audit_memory_id = if accept {
        Some(digest(&format!(
            "resolved\0{}\0{}\0{}",
            conflict_id, incoming.0, incoming.1
        )))
    } else {
        None
    };
    tx.execute(
        "INSERT INTO audit_log(action,conflict_id,memory_id,details,created_at) VALUES (?1,?2,?3,?4,?5)",
        params![resolution, conflict_id, audit_memory_id, reason, now()],
    )?;
    tx.commit()?;
    if accept {
        recover_conflict_outbox(conn, home)?;
    }
    Ok(())
}

pub fn search(conn: &Connection, query: &str, scope: Option<&str>, limit: u32) -> Result<Vec<Hit>> {
    let tokens: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .take(12)
        .map(|t| format!("\"{}\"", t))
        .collect();
    if tokens.is_empty() || limit == 0 {
        return Ok(vec![]);
    }
    let fts = tokens.join(" OR ");
    let mut hits = Vec::new();
    {
        let mut q = conn.prepare("SELECT m.id,m.kind,m.scope,m.text FROM memories_fts f JOIN memories m ON m.rowid=f.rowid WHERE memories_fts MATCH ?1 AND m.status='active' AND (?2 IS NULL OR m.scope=?2) ORDER BY bm25(memories_fts) LIMIT ?3")?;
        let rows = q.query_map(params![fts, scope, limit], |r| {
            Ok(Hit {
                id: r.get(0)?,
                kind: r.get(1)?,
                scope: r.get(2)?,
                text: r.get(3)?,
            })
        })?;
        for row in rows {
            hits.push(row?);
        }
    }
    if hits.len() < limit as usize {
        let remaining = limit as usize - hits.len();
        let mut q = conn.prepare("SELECT e.id,e.scope,e.text FROM events_fts f JOIN events e ON e.rowid=f.rowid WHERE events_fts MATCH ?1 AND (?2 IS NULL OR e.scope=?2) ORDER BY bm25(events_fts) LIMIT ?3")?;
        let rows = q.query_map(params![fts, scope, remaining as i64], |r| {
            Ok(Hit {
                id: r.get(0)?,
                kind: "episode".into(),
                scope: r.get(1)?,
                text: r.get(2)?,
            })
        })?;
        for row in rows {
            hits.push(row?);
        }
    }
    Ok(hits)
}

// Recovery after file/database divergence. Markdown memory files are canonical for durable memories.
pub fn reindex(conn: &mut Connection, home: &Path) -> Result<usize> {
    {
        let mut query = conn.prepare("SELECT file_path FROM memories")?;
        let paths = query.query_map([], |row| row.get::<_, String>(0))?;
        for path in paths {
            let path = path?;
            if !Path::new(&path).is_file() {
                return Err(format!("cannot reindex: memory file is missing: {path}").into());
            }
        }
    }
    let mut docs = Vec::new();
    for entry in fs::read_dir(home.join("memories"))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let data = fs::read_to_string(&path)?;
        let first = data.lines().next().ok_or("empty memory file")?;
        let raw = first
            .strip_prefix("<!-- plastia:")
            .and_then(|s| s.strip_suffix(" -->"))
            .ok_or("invalid memory header")?;
        let v: serde_json::Value = serde_json::from_str(raw)?;
        let field = |k: &str| -> Result<String> {
            Ok(v[k]
                .as_str()
                .ok_or("missing memory metadata field")?
                .to_owned())
        };
        let content = data
            .split_once("\n\n")
            .and_then(|(_, after)| after.split_once("\n\n"))
            .map(|(_, body)| body.trim().to_string())
            .ok_or("invalid Markdown memory")?;
        docs.push((
            field("id")?,
            field("scope")?,
            field("kind")?,
            field("key")?,
            content,
            field("source_event")?,
            path.to_string_lossy().into_owned(),
            field("created_at")?,
        ));
    }
    let mut lifecycle = HashMap::new();
    {
        let mut query = conn.prepare(
            "SELECT id,status,supersedes,valid_from,valid_until,evidence_type FROM memories",
        )?;
        let rows = query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        for row in rows {
            let (id, status, supersedes, valid_from, valid_until, evidence_type) = row?;
            lifecycle.insert(
                id,
                (status, supersedes, valid_from, valid_until, evidence_type),
            );
        }
    }
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM memories", [])?;
    for (id, scope, kind, key, text, source, path, created) in &docs {
        let (status, supersedes, valid_from, valid_until, evidence_type) = lifecycle
            .get(id)
            .cloned()
            .unwrap_or(("active".into(), None, None, None, "observed".into()));
        tx.execute("INSERT INTO memories(id,scope,kind,memory_key,text,source_event,status,supersedes,valid_from,valid_until,evidence_type,file_path,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![id,scope,kind,key,text,source,status,supersedes,valid_from,valid_until,evidence_type,path,created])?;
    }
    tx.commit()?;
    Ok(docs.len())
}

pub fn stats(conn: &Connection) -> Result<serde_json::Value> {
    let count = |table: &str| -> rusqlite::Result<i64> {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
    };
    Ok(
        json!({"events":count("events")?, "unprocessed_events":count("(SELECT 1 FROM events WHERE processed=0)")?, "memories":count("memories")?, "pending_conflicts":count("(SELECT 1 FROM conflicts WHERE resolution='pending')")?}),
    )
}
