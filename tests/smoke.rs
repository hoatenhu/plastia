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
