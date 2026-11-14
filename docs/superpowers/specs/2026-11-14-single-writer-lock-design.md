# Single-Writer Lock Design

## Intent

Serialize Plastia commands that modify the store so SQLite state and Markdown
files are never changed by competing Plastia writers at the same time.

## Approach

Use an exclusive lock file at `<home>/write.lock`. A mutating command creates
the file with `create_new`, writes JSON metadata containing the current PID and
start time, then holds the file handle until the command completes. Search and
stats remain lock-free because they do not modify the store.

If the lock exists, read its metadata. A live owner returns a busy error and
does not modify the store. A stale owner whose PID no longer exists may be
removed and retried once. Invalid metadata is an error; never delete an
unverifiable lock automatically.

## Lock scope

- Locked: `init`, `ingest`, `consolidate`, `reindex`.
- Unlocked: `search`, `stats`.

The lock protects the complete command, including outbox recovery, SQLite
transactions and Markdown file operations. The existing outbox remains the
crash-recovery mechanism; the lock prevents concurrent writers but does not
provide power-loss guarantees or protect manual edits outside Plastia.

## Acceptance criteria

- Two concurrent mutating commands cannot both acquire the lock.
- A live lock is never removed by another process.
- A lock with a dead PID is recovered safely.
- Invalid lock metadata fails closed.
- Normal command completion removes the lock.
- Lock acquisition and release do not change read-only command behavior.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test` pass.
