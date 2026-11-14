# M1 SQLite Outbox Design

## Intent

Make consolidation recoverable when the process stops between writing a
Markdown memory and materializing its SQLite index. Preserve the existing
contract: Markdown remains the durable human-readable representation, while
SQLite owns transactional state and indexes.

## Goals

- Persist every memory-write intent before applying it.
- Replay unfinished intents safely after restart.
- Keep memory creation idempotent across crashes and duplicate replay.
- Ensure an acknowledged event is not marked processed before its derived
  memory work is durable or safely replayable.
- Add failure-injection tests for interruption at each write boundary.

## Non-goals

- Multi-process writer locking in this first slice.
- Full conflict-resolution authority policies.
- Encryption, retention, or remote synchronization.
- Claiming directory `fsync` crash guarantees before they are tested.

## Approach

Add a SQLite `outbox` table containing a stable intent ID, operation kind,
memory identity, serialized Markdown payload, target path, source event, and
status. Consolidation creates the outbox intent inside the SQLite transaction
that reserves the event. A replay/apply step writes the Markdown through a
temporary file and rename, then inserts the memory row and marks the outbox
intent applied in one SQLite transaction. Existing rows and files make replay
idempotent.

The event is marked processed only after all candidate intents for that event
are applied. Dry-run creates neither outbox rows nor files and does not mark
events processed.

## Recovery flow

```text
read pending outbox intents
  -> verify or create target Markdown file
  -> materialize missing SQLite memory row
  -> mark intent applied
  -> mark source event processed when no pending intents remain
```

If the process stops after the file rename but before the SQLite commit,
replay recognizes the deterministic target and inserts the missing row. If it
stops before the rename, replay writes the file. If the SQL transaction has
committed, replay skips the applied intent.

## Acceptance criteria

- Fresh consolidation produces the same Markdown and memory counts as today.
- Re-running consolidation after every injected interruption point produces one
  memory per deterministic identity and leaves no pending outbox rows.
- Duplicate event replay does not create duplicate outbox intents or memories.
- Dry-run leaves events, outbox, memories, and files unchanged.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test` pass.

## Deliberate ceiling

This slice uses SQLite transactions and atomic rename but does not claim full
power-loss durability until directory sync and fault-injection behavior are
measured. A later M1 slice can add single-writer locking and stronger fsync
recovery if tests demonstrate the need.
