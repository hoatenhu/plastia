use crate::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
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
                        conn.execute(
                            "INSERT OR IGNORE INTO outbox(id,event_id,scope,kind,memory_key,memory_id,text,markdown,file_path,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                            params![intent_id, event_id, scope, c.kind, c.key, id, c.text, md, path.to_string_lossy().to_string(), timestamp],
                        )?;
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
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM memories", [])?;
    for (id, scope, kind, key, text, source, path, created) in &docs {
        tx.execute("INSERT INTO memories(id,scope,kind,memory_key,text,source_event,file_path,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![id,scope,kind,key,text,source,path,created])?;
    }
    tx.commit()?;
    Ok(docs.len())
}

pub fn stats(conn: &Connection) -> Result<serde_json::Value> {
    let count = |table: &str| -> rusqlite::Result<i64> {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
    };
    Ok(
        json!({"events":count("events")?, "unprocessed_events":count("(SELECT 1 FROM events WHERE processed=0)")?, "memories":count("memories")?, "pending_conflicts":count("(SELECT 1 FROM conflicts WHERE status='pending')")?}),
    )
}
