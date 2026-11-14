# M1 SQLite Outbox Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make consolidation recoverable across interruptions between Markdown writes and SQLite materialization.

**Architecture:** Add a SQLite `outbox` table for deterministic memory-write intents. Consolidation reserves work transactionally, applies pending intents through atomic Markdown replacement plus idempotent SQLite materialization, then marks source events processed only after all intents apply. Recovery replays pending intents on startup or before consolidation.

**Tech Stack:** Rust stable, SQLite via `rusqlite`, Markdown files, existing CLI and integration tests.

**Spec:** `docs/superpowers/specs/2026-11-14-m1-sqlite-outbox-design.md`

## Global Constraints

- Preserve append-only source events and attribution; never silently overwrite or elevate authority.
- SQLite is canonical for the event log and transactional state; Markdown is canonical for durable human-readable knowledge.
- Writes must be idempotent and recoverable after crash or replay.
- Dry-run must not modify events, outbox, memories, or files.
- Do not claim full power-loss durability until directory sync and recovery are tested.
- Keep the core offline and do not add a dependency for functionality already covered by the standard library or existing dependencies.

## Review Focus

- Process interruption after Markdown rename but before SQLite materialization must replay without duplicate memories.
- Process interruption before Markdown rename must leave a pending intent that replay can complete.
- A malformed or inconsistent outbox payload must fail without marking its source event processed.
- Existing stores without an `outbox` table must open through an idempotent schema upgrade.
- Repeated replay and duplicate event ingestion must not create duplicate intents, files, or memory rows.

---

### Task 1: Add outbox schema and deterministic intent model

**Files:**
- Modify: `src/storage.rs`
- Modify: `src/engine.rs`
- Test: `tests/smoke.rs`

**Interfaces:**
- Consumes: existing `storage::open`, `engine::ingest_file`, `engine::consolidate`.
- Produces: outbox schema with stable intent identity and helpers used by Tasks 2–4.

- [ ] **Step 1: Write the failing schema-upgrade test**

Add `opens_existing_store_with_outbox_schema` to create a store with the
current tables, reopen it through `storage::open`, and assert that
`SELECT COUNT(*) FROM outbox` succeeds.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test opens_existing_store_with_outbox_schema -- --nocapture`

Expected: FAIL because `outbox` does not exist.

- [ ] **Step 3: Add the idempotent `outbox` schema in `src/storage.rs`**

Create columns for `id TEXT PRIMARY KEY`, `event_id`, `scope`, `kind`,
`memory_key`, `memory_id`, `text`, `markdown`, `file_path`, `created_at`, and
`status TEXT NOT NULL DEFAULT 'pending'`, plus indexes for pending status and
source event. Keep creation in the existing `execute_batch` migration path.

- [ ] **Step 4: Add the deterministic intent identity helper**

Implement a private helper in `src/engine.rs` that derives the outbox ID from
`scope`, candidate kind/key/text and source event ID. The same input must always
produce the same ID.

- [ ] **Step 5: Run the focused test to verify it passes**

Run: `cargo test opens_existing_store_with_outbox_schema -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/storage.rs src/engine.rs tests/smoke.rs
git commit -m "feat: add consolidation outbox schema"
```

### Task 2: Persist memory intents before applying files

**Files:**
- Modify: `src/engine.rs`
- Test: `tests/smoke.rs`

**Interfaces:**
- Consumes: Task 1 outbox schema and deterministic intent ID helper.
- Produces: `consolidate` inserts pending intents before applying durable memory files and leaves dry-run unchanged.

- [ ] **Step 1: Write the failing outbox behavior test**

Add `consolidate_persists_pending_intent_before_materialization` that ingests
one explicit decision, runs consolidation, and asserts one applied outbox row,
one memory row, one Markdown file, and a processed event. Extend the dry-run
test to assert the outbox count remains zero.

- [ ] **Step 2: Run the tests to verify the new assertions fail**

Run: `cargo test consolidate_persists_pending_intent_before_materialization dry_run_preserves_work_and_scope_isolation -- --nocapture`

Expected: FAIL because consolidation currently has no outbox rows.

- [ ] **Step 3: Implement intent insertion and application**

Refactor the memory-creation branch so the outbox intent is inserted with
`INSERT OR IGNORE` before Markdown materialization. Apply the intent using the
existing deterministic file path and `atomic_write`, then insert the memory
row with `INSERT OR IGNORE` and mark the intent `applied`. Do not create outbox
rows during dry-run.

- [ ] **Step 4: Mark source events processed only after applied work**

Keep the existing event update, but make it conditional on no pending outbox
intent for that event. Existing duplicate/conflict behavior must remain
unchanged.

- [ ] **Step 5: Run focused and full tests**

Run: `cargo test consolidate_persists_pending_intent_before_materialization dry_run_preserves_work_and_scope_isolation -- --nocapture`

Expected: PASS.

Run: `cargo test`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/engine.rs tests/smoke.rs
git commit -m "feat: persist consolidation intents"
```

### Task 3: Replay pending intents on recovery

**Files:**
- Modify: `src/engine.rs`
- Modify: `src/main.rs`
- Test: `tests/smoke.rs`

**Interfaces:**
- Consumes: Task 2 pending/applied outbox records and deterministic target paths.
- Produces: `engine::recover(conn, home) -> Result<usize>` and automatic recovery before mutating CLI commands.

- [ ] **Step 1: Write the failing replay test**

Add `recovery_replays_pending_intent_idempotently`: create an event and a
pending outbox row pointing at a deterministic Markdown payload, run
`reindex` or the recovery path, and assert the file, memory row, applied status,
and processed event. Run recovery twice and assert counts remain one.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test recovery_replays_pending_intent_idempotently -- --nocapture`

Expected: FAIL because no recovery function exists.

- [ ] **Step 3: Implement `engine::recover`**

Read pending outbox rows in stable order. For each row, validate required
metadata, create the target Markdown file with `atomic_write` if missing, then
insert the memory row idempotently and mark the outbox row applied in one SQL
transaction. Mark the source event processed only when no pending intent
remains. Return the number of intents applied.

- [ ] **Step 4: Invoke recovery before mutating operations**

Call `recover` after opening the store and before `ingest`, `consolidate`, and
`reindex` work that depends on consistent durable memory state. Keep `search`,
`stats`, and `init` read-only or initialization-only as currently defined.

- [ ] **Step 5: Run focused and full tests**

Run: `cargo test recovery_replays_pending_intent_idempotently -- --nocapture`

Expected: PASS.

Run: `cargo test`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/engine.rs src/main.rs tests/smoke.rs
git commit -m "feat: replay pending consolidation intents"
```

### Task 4: Add interruption-boundary coverage and update docs

**Files:**
- Modify: `src/engine.rs`
- Modify: `tests/smoke.rs`
- Modify: `README.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/ARCHITECTURE.md`

**Interfaces:**
- Consumes: Task 3 `engine::recover` and outbox lifecycle.
- Produces: documented M1 behavior and regression coverage for file/SQL divergence.

- [ ] **Step 1: Write failing interruption-boundary tests**

Add tests for a pending intent with no Markdown file and a pending intent with
an already-written Markdown file but no memory row. Both must recover to one
applied intent and one memory without overwriting conflicting existing files.

- [ ] **Step 2: Run the tests to verify the intended failures**

Run: `cargo test recovery_ -- --nocapture`

Expected: any unsupported boundary behavior fails before implementation.

- [ ] **Step 3: Implement the minimum boundary handling**

Use deterministic content/path checks during recovery. If an existing target
file differs from the intent payload, return an error and leave the intent
pending; never silently overwrite unrelated content.

- [ ] **Step 4: Update documentation**

Document the outbox/replay behavior, the current single-writer limitation,
and the deliberate lack of a full directory-fsync guarantee. Mark only the
completed M1 acceptance items in the roadmap.

- [ ] **Step 5: Run all verification gates**

Run:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: all commands pass.

- [ ] **Step 6: Commit**

```bash
git add src/engine.rs tests/smoke.rs README.md docs/ROADMAP.md docs/ARCHITECTURE.md
git commit -m "test: cover consolidation recovery boundaries"
```

## Checkpoint: M1 outbox slice

- [ ] Existing smoke flow remains green.
- [ ] Pending intents replay idempotently after simulated interruption.
- [ ] Dry-run leaves all durable state unchanged.
- [ ] Conflicting existing Markdown is not silently overwritten.
- [ ] Formatting, clippy and tests pass.

## Deferred M1 work

- Single-writer file/process locking.
- Kill-process fault injection rather than deterministic boundary simulation.
- Directory `fsync` and stronger power-loss guarantees.
- Authority-verified conflict resolution and version history.
