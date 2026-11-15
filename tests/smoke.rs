use rusqlite::Connection;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn capture_search_consolidate_and_conflict() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let run = |args: &[&str]| {
        let out = Command::new(bin)
            .arg("--home")
            .arg(&home)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    run(&["init"]);
    run(&["ingest", "examples/conversation.json"]);
    assert!(run(&["search", "production approval"]).contains("production"));
    let preview = run(&["consolidate", "--dry-run"]);
    assert!(preview.contains("\"dry_run\": true"));
    assert!(run(&["stats"]).contains("\"memories\": 0"));
    let result = run(&["consolidate"]);
    assert!(result.contains("\"conflicts\": 1"));
    assert!(run(&["search", "CEO", "--scope", "project/swich"]).contains("CEO"));
    assert!(run(&["stats"]).contains("\"pending_conflicts\": 1"));
    assert!(run(&["reindex"]).contains("Indexed 3"));
    let result = run(&["consolidate"]);
    assert!(result.contains("\"events_reviewed\": 0"));
}

#[test]
fn malformed_batch_is_atomic_and_duplicate_replay_is_idempotent() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let run = |args: &[&str]| {
        let out = Command::new(bin)
            .arg("--home")
            .arg(&home)
            .args(args)
            .output()
            .unwrap();
        (
            out.status.success(),
            String::from_utf8(out.stdout).unwrap(),
            String::from_utf8(out.stderr).unwrap(),
        )
    };
    run(&["init"]);

    let malformed = tmp.path().join("malformed.json");
    fs::write(
        &malformed,
        r#"[{"id":"valid","text":"Decision: gate = review"},{"text":""}]"#,
    )
    .unwrap();
    let malformed_path = malformed.to_str().unwrap();
    let (success, _, stderr) = run(&["ingest", malformed_path]);
    assert!(!success);
    assert!(stderr.contains("event text/scope must not be empty"));
    assert!(run(&["stats"]).1.contains("\"events\": 0"));

    let valid = tmp.path().join("valid.json");
    fs::write(
        &valid,
        r#"[{"id":"stable","scope":"project/a","text":"Decision: gate = review"}]"#,
    )
    .unwrap();
    let valid_path = valid.to_str().unwrap();
    assert!(run(&["ingest", valid_path]).1.contains("Ingested 1"));
    assert!(run(&["ingest", valid_path]).1.contains("Ingested 0"));
    assert!(run(&["stats"]).1.contains("\"events\": 1"));
}

#[test]
fn dry_run_preserves_work_and_scope_isolation() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let run = |args: &[&str]| {
        let out = Command::new(bin)
            .arg("--home")
            .arg(&home)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    run(&["init"]);
    let events = tmp.path().join("events.json");
    fs::write(
        &events,
        r#"{"events":[
      {"id":"a","scope":"project/a","text":"Decision: gate = review"},
      {"id":"b","scope":"project/b","text":"Decision: gate = deploy"}
    ]}"#,
    )
    .unwrap();
    let events_path = events.to_str().unwrap();
    run(&["ingest", events_path]);
    assert!(run(&["consolidate", "--dry-run"]).contains("\"memories_created\": 2"));
    assert!(run(&["stats"]).contains("\"memories\": 0"));
    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let outbox: i64 = conn
        .query_row("SELECT COUNT(*) FROM outbox", [], |row| row.get(0))
        .unwrap();
    assert_eq!(outbox, 0);
    drop(conn);
    run(&["consolidate"]);
    assert!(run(&["search", "review", "--scope", "project/a"]).contains("review"));
    assert!(!run(&["search", "review", "--scope", "project/b"]).contains("review"));
    assert!(run(&["search", "deploy", "--scope", "project/b"]).contains("deploy"));
    assert!(run(&["consolidate"]).contains("\"events_reviewed\": 0"));
}

#[test]
fn opens_existing_store_with_outbox_schema() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let init = Command::new(bin)
        .arg("--home")
        .arg(&home)
        .arg("init")
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    conn.execute("DROP TABLE outbox", []).unwrap();
    drop(conn);

    let reopened = Command::new(bin)
        .arg("--home")
        .arg(&home)
        .arg("stats")
        .output()
        .unwrap();
    assert!(
        reopened.status.success(),
        "{}",
        String::from_utf8_lossy(&reopened.stderr)
    );

    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM outbox", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn consolidate_persists_pending_intent_before_materialization() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let run = |args: &[&str]| {
        let out = Command::new(bin)
            .arg("--home")
            .arg(&home)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    run(&["init"]);

    let event_file = tmp.path().join("event.json");
    fs::write(
        &event_file,
        r#"{"id":"decision-1","scope":"project/a","text":"Decision: gate = review"}"#,
    )
    .unwrap();
    run(&["ingest", event_file.to_str().unwrap()]);
    run(&["consolidate"]);

    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let applied: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM outbox WHERE status='applied'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let memories: i64 = conn
        .query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))
        .unwrap();
    let processed: i64 = conn
        .query_row("SELECT COUNT(*) FROM events WHERE processed=1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!((applied, memories, processed), (1, 1, 1));
    assert_eq!(fs::read_dir(home.join("memories")).unwrap().count(), 1);
}

#[test]
fn recovery_replays_pending_intent_idempotently() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let run = |args: &[&str]| {
        let out = Command::new(bin)
            .arg("--home")
            .arg(&home)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    run(&["init"]);

    let event_file = tmp.path().join("event.json");
    fs::write(
        &event_file,
        r#"{"id":"decision-1","scope":"project/a","text":"Decision: gate = review"}"#,
    )
    .unwrap();
    run(&["ingest", event_file.to_str().unwrap()]);

    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let memory_id = "memory-recovery";
    let file_path = home.join("memories").join(format!("{memory_id}.md"));
    let markdown = "<!-- plastia:{} -->\n\n# decision\n\nreview\n";
    conn.execute(
        "INSERT INTO outbox(id,event_id,scope,kind,memory_key,memory_id,text,markdown,file_path,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        rusqlite::params!["intent-recovery", "decision-1", "project/a", "decision", "gate", memory_id, "review", markdown, file_path.to_string_lossy().to_string(), "now"],
    )
    .unwrap();
    drop(conn);

    run(&["consolidate"]);
    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let applied: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM outbox WHERE status='applied'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(applied, 1);
    assert_eq!(fs::read_to_string(&file_path).unwrap(), markdown);
    let memories: i64 = conn
        .query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))
        .unwrap();
    assert_eq!(memories, 1);
    drop(conn);

    run(&["consolidate"]);
    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let (outbox, memories): (i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM outbox WHERE status='applied'), (SELECT COUNT(*) FROM memories)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((outbox, memories), (1, 1));
}

#[test]
fn recovery_rejects_conflicting_existing_memory_file() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let run = |args: &[&str]| {
        let out = Command::new(bin)
            .arg("--home")
            .arg(&home)
            .args(args)
            .output()
            .unwrap();
        (
            out.status.success(),
            String::from_utf8(out.stdout).unwrap(),
            String::from_utf8(out.stderr).unwrap(),
        )
    };
    assert!(run(&["init"]).0);
    let event_file = tmp.path().join("event.json");
    fs::write(
        &event_file,
        r#"{"id":"decision-1","scope":"project/a","text":"Decision: gate = review"}"#,
    )
    .unwrap();
    assert!(run(&["ingest", event_file.to_str().unwrap()]).0);

    let path = home.join("memories/conflicting.md");
    let expected = "expected content";
    fs::write(&path, "unrelated content").unwrap();
    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    conn.execute(
        "INSERT INTO outbox(id,event_id,scope,kind,memory_key,memory_id,text,markdown,file_path,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        rusqlite::params!["intent-conflict", "decision-1", "project/a", "decision", "gate", "memory-conflict", expected, expected, path.to_string_lossy().to_string(), "now"],
    ).unwrap();
    drop(conn);

    let (success, _, stderr) = run(&["consolidate"]);
    assert!(!success);
    assert!(stderr.contains("outbox target differs"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "unrelated content");
    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let status: String = conn
        .query_row(
            "SELECT status FROM outbox WHERE id='intent-conflict'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let processed: i64 = conn
        .query_row(
            "SELECT processed FROM events WHERE id='decision-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((status, processed), ("pending".into(), 0));
}

#[test]
fn writer_lock_rejects_live_owner_and_reclaims_dead_owner() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let run = |args: &[&str]| {
        Command::new(bin)
            .arg("--home")
            .arg(&home)
            .args(args)
            .output()
            .unwrap()
    };
    let init = run(&["init"]);
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(!home.join("write.lock").exists());

    let current_pid = std::process::id();
    fs::write(
        home.join("write.lock"),
        format!(r#"{{"pid":{current_pid},"started_at":0}}"#),
    )
    .unwrap();
    let blocked = run(&["ingest", "examples/conversation.json"]);
    assert!(!blocked.status.success());
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("store is busy"));
    assert!(home.join("write.lock").exists());

    fs::write(
        home.join("write.lock"),
        r#"{"pid":4294967295,"started_at":0}"#,
    )
    .unwrap();
    let recovered = run(&["consolidate"]);
    assert!(
        recovered.status.success(),
        "{}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    assert!(!home.join("write.lock").exists());
}

#[test]
fn writer_lock_rejects_invalid_metadata_and_read_only_commands_ignore_lock() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let run = |args: &[&str]| {
        Command::new(bin)
            .arg("--home")
            .arg(&home)
            .args(args)
            .output()
            .unwrap()
    };
    let init = run(&["init"]);
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    fs::write(home.join("write.lock"), "not-json").unwrap();

    let blocked = run(&["ingest", "examples/conversation.json"]);
    assert!(!blocked.status.success());
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("invalid writer lock"));
    assert!(home.join("write.lock").exists());

    let stats = run(&["stats"]);
    assert!(
        stats.status.success(),
        "{}",
        String::from_utf8_lossy(&stats.stderr)
    );
    let search = run(&["search", "production"]);
    assert!(
        search.status.success(),
        "{}",
        String::from_utf8_lossy(&search.stderr)
    );
}

#[test]
fn process_exit_after_outbox_commit_recovers() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let run = |args: &[&str]| {
        Command::new(bin)
            .arg("--home")
            .arg(&home)
            .args(args)
            .output()
            .unwrap()
    };
    let init = run(&["init"]);
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    let event_file = tmp.path().join("event.json");
    fs::write(
        &event_file,
        r#"{"id":"decision-failpoint","scope":"project/a","text":"Decision: gate = review"}"#,
    )
    .unwrap();
    let ingest = run(&["ingest", event_file.to_str().unwrap()]);
    assert!(
        ingest.status.success(),
        "{}",
        String::from_utf8_lossy(&ingest.stderr)
    );

    let interrupted = Command::new(bin)
        .arg("--home")
        .arg(&home)
        .arg("consolidate")
        .env("PLASTIA_FAILPOINT", "after-outbox")
        .output()
        .unwrap();
    assert_eq!(interrupted.status.code(), Some(86));
    assert!(home.join("write.lock").exists());
    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let pending: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM outbox WHERE status='pending'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let memories: i64 = conn
        .query_row("SELECT COUNT(*) FROM memories", [], |row| row.get(0))
        .unwrap();
    assert_eq!((pending, memories), (1, 0));
    drop(conn);

    let recovered = run(&["consolidate"]);
    assert!(
        recovered.status.success(),
        "{}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    let repeated = run(&["consolidate"]);
    assert!(
        repeated.status.success(),
        "{}",
        String::from_utf8_lossy(&repeated.stderr)
    );
    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let counts: (i64, i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM outbox WHERE status='applied'), (SELECT COUNT(*) FROM memories), (SELECT COUNT(*) FROM events WHERE processed=1)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(counts, (1, 1, 1));
    assert_eq!(fs::read_dir(home.join("memories")).unwrap().count(), 1);
}

#[test]
fn memory_lifecycle_schema_migrates_existing_store() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let init = Command::new(bin)
        .arg("--home")
        .arg(&home)
        .arg("init")
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );

    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let event_id = "legacy-event";
    conn.execute(
        "INSERT INTO events(id,scope,source,occurred_at,text) VALUES (?1,?2,?3,?4,?5)",
        rusqlite::params![event_id, "project/a", "test", "now", "legacy"],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO memories(id,scope,kind,memory_key,text,source_event,file_path,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        rusqlite::params!["legacy-memory", "project/a", "decision", "gate", "review", event_id, "legacy.md", "now"],
    )
    .unwrap();
    drop(conn);

    let stats = Command::new(bin)
        .arg("--home")
        .arg(&home)
        .arg("stats")
        .output()
        .unwrap();
    assert!(
        stats.status.success(),
        "{}",
        String::from_utf8_lossy(&stats.stderr)
    );

    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let columns: Vec<String> = conn
        .prepare("PRAGMA table_info(memories)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .map(|row| row.unwrap())
        .collect();
    assert!(columns.iter().any(|column| column == "supersedes"));
    assert!(columns.iter().any(|column| column == "valid_from"));
    assert!(columns.iter().any(|column| column == "evidence_type"));
    let lifecycle: (String, Option<String>, String) = conn
        .query_row(
            "SELECT status, supersedes, evidence_type FROM memories WHERE id='legacy-memory'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(lifecycle, ("active".into(), None, "observed".into()));
    let audit: i64 = conn
        .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
        .unwrap();
    assert_eq!(audit, 0);
}

#[test]
fn consolidated_memory_preserves_lifecycle_and_provenance_on_reindex() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let run = |args: &[&str]| {
        let out = Command::new(bin)
            .arg("--home")
            .arg(&home)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    run(&["init"]);
    let event_file = tmp.path().join("event.json");
    fs::write(
        &event_file,
        r#"{"id":"provenance-event","scope":"project/a","source":"untrusted/document","occurred_at":"2026-11-15T00:00:00Z","text":"Decision: gate = review"}"#,
    )
    .unwrap();
    run(&["ingest", event_file.to_str().unwrap()]);
    run(&["consolidate"]);

    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let before: (String, String, String, String, String) = conn
        .query_row(
            "SELECT status, source_event, evidence_type, created_at, file_path FROM memories",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(before.0, "active");
    assert_eq!(before.1, "provenance-event");
    assert_eq!(before.2, "observed");
    drop(conn);

    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    conn.execute(
        "UPDATE memories SET status='superseded', supersedes='older-memory', valid_from='2026-11-15', valid_until='2026-11-16'",
        [],
    )
    .unwrap();
    drop(conn);
    run(&["reindex"]);
    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let after: (String, Option<String>, Option<String>, Option<String>, String, String, String) = conn
        .query_row(
            "SELECT status, supersedes, valid_from, valid_until, source_event, evidence_type, created_at FROM memories",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
        )
        .unwrap();
    assert_eq!(after.0, "superseded");
    assert_eq!(after.1, Some("older-memory".into()));
    assert_eq!(after.2, Some("2026-11-15".into()));
    assert_eq!(after.3, Some("2026-11-16".into()));
    assert_eq!(after.4, before.1);
    assert_eq!(after.5, before.2);
    assert_eq!(after.6, before.3);
}

#[test]
fn conflict_accept_creates_active_version_and_is_idempotent() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let run = |args: &[&str]| {
        let out = Command::new(bin)
            .arg("--home")
            .arg(&home)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    run(&["init"]);
    let events = tmp.path().join("events.json");
    fs::write(
        &events,
        r#"{"events":[
          {"id":"old","scope":"project/a","text":"Decision: gate = review"},
          {"id":"new","scope":"project/a","text":"Decision: gate = deploy"}
        ]}"#,
    )
    .unwrap();
    run(&["ingest", events.to_str().unwrap()]);
    run(&["consolidate"]);
    let conflicts = run(&["conflicts"]);
    let conflict_id = conflicts
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    run(&[
        "resolve-conflict",
        &conflict_id,
        "--accept",
        "--reason",
        "approved correction",
    ]);
    run(&[
        "resolve-conflict",
        &conflict_id,
        "--accept",
        "--reason",
        "repeat",
    ]);

    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let active: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE status='active'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let superseded: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE status='superseded'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let accepted: String = conn
        .query_row(
            "SELECT resolution FROM conflicts WHERE id=?1",
            [&conflict_id],
            |row| row.get(0),
        )
        .unwrap();
    let audits: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_log WHERE conflict_id=?1",
            [&conflict_id],
            |row| row.get(0),
        )
        .unwrap();
    let audit_memory: Option<String> = conn
        .query_row(
            "SELECT memory_id FROM audit_log WHERE conflict_id=?1",
            [&conflict_id],
            |row| row.get(0),
        )
        .unwrap();
    let active_memory: String = conn
        .query_row("SELECT id FROM memories WHERE status='active'", [], |row| {
            row.get(0)
        })
        .unwrap();
    drop(conn);
    let stats: serde_json::Value = serde_json::from_str(&run(&["stats"])).unwrap();
    assert_eq!(
        (active, superseded, accepted, audits),
        (1, 1, "accepted".into(), 1)
    );
    assert_eq!(audit_memory, Some(active_memory));
    assert_eq!(stats["pending_conflicts"], 0);
}

#[test]
fn conflict_reject_preserves_existing_active_memory() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let run = |args: &[&str]| {
        let out = Command::new(bin)
            .arg("--home")
            .arg(&home)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    run(&["init"]);
    let events = tmp.path().join("events.json");
    fs::write(&events, r#"{"events":[{"id":"old","scope":"project/a","text":"Decision: gate = review"},{"id":"new","scope":"project/a","text":"Decision: gate = deploy"}]}"#).unwrap();
    run(&["ingest", events.to_str().unwrap()]);
    run(&["consolidate"]);
    let conflict_id = run(&["conflicts"])
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    run(&[
        "resolve-conflict",
        &conflict_id,
        "--reject",
        "--reason",
        "untrusted source",
    ]);
    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let active_text: String = conn
        .query_row(
            "SELECT text FROM memories WHERE status='active'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(active_text, "review");
    let active_deploy: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE status='active' AND text='deploy'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(active_deploy, 0);
    drop(conn);
    let stats: serde_json::Value = serde_json::from_str(&run(&["stats"])).unwrap();
    assert_eq!(stats["pending_conflicts"], 0);
}

#[test]
fn reindex_refuses_to_delete_memory_when_markdown_is_missing() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let run = |args: &[&str]| {
        let out = Command::new(bin)
            .arg("--home")
            .arg(&home)
            .args(args)
            .output()
            .unwrap();
        out
    };
    assert!(run(&["init"]).status.success());
    let event = tmp.path().join("event.json");
    fs::write(
        &event,
        r#"{"id":"missing-file-event","scope":"project/a","text":"Decision: gate = review"}"#,
    )
    .unwrap();
    assert!(run(&["ingest", event.to_str().unwrap()]).status.success());
    assert!(run(&["consolidate"]).status.success());
    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let path: String = conn
        .query_row("SELECT file_path FROM memories", [], |row| row.get(0))
        .unwrap();
    let memory_id: String = conn
        .query_row("SELECT id FROM memories", [], |row| row.get(0))
        .unwrap();
    drop(conn);
    fs::remove_file(path).unwrap();
    let reindex = run(&["reindex"]);
    assert!(!reindex.status.success());
    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let preserved: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memories WHERE id=?1",
            [&memory_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(preserved, 1);
}

#[test]
fn policy_conflict_accept_is_rejected_without_authority() {
    let tmp = tempdir().unwrap();
    let home = tmp.path().join("store");
    let bin = env!("CARGO_BIN_EXE_plastia");
    let run = |args: &[&str]| {
        let out = Command::new(bin)
            .arg("--home")
            .arg(&home)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    };
    run(&["init"]);
    let events = tmp.path().join("events.json");
    fs::write(&events, r#"{"events":[{"id":"policy-old","scope":"project/a","text":"Policy: deploy = review"},{"id":"policy-new","scope":"project/a","text":"Policy: deploy = automatic"}]}"#).unwrap();
    run(&["ingest", events.to_str().unwrap()]);
    run(&["consolidate"]);
    let conflict_id = run(&["conflicts"])
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    let out = Command::new(bin)
        .arg("--home")
        .arg(&home)
        .args([
            "resolve-conflict",
            &conflict_id,
            "--accept",
            "--reason",
            "approved",
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let conn = Connection::open(home.join("memory.sqlite3")).unwrap();
    let resolution: String = conn
        .query_row(
            "SELECT resolution FROM conflicts WHERE id=?1",
            [&conflict_id],
            |row| row.get(0),
        )
        .unwrap();
    let active: i64 = conn.query_row("SELECT COUNT(*) FROM memories WHERE status='active' AND kind='policy' AND text='automatic'", [], |row| row.get(0)).unwrap();
    assert_eq!(resolution, "pending");
    assert_eq!(active, 0);
}
