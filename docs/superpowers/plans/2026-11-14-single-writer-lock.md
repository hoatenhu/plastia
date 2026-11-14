# Single-Writer Lock Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Serialize Plastia mutating commands with a safe PID-bearing lock file.

**Architecture:** Add `storage::WriterLock`, acquired with exclusive file creation and held until command completion. Live owners block with a busy error; dead owners may be reclaimed; invalid metadata fails closed. Read-only commands remain lock-free.

**Tech Stack:** Rust stable standard library, existing CLI and integration tests.

**Spec:** `docs/superpowers/specs/2026-11-14-single-writer-lock-design.md`

## Global Constraints

- Locked: `init`, `ingest`, `consolidate`, `reindex`.
- Unlocked: `search`, `stats`.
- A live lock is never removed by another process.
- Invalid lock metadata fails closed.
- Do not add a dependency for file locking.
- Existing outbox remains the crash-recovery mechanism.

## Review Focus

- Two writers starting together must not both enter mutating work.
- A malformed lock must not be deleted automatically.
- A dead PID lock must be reclaimable.
- Normal errors and success must release the lock.
- Read-only commands must work while a writer lock exists.

---

### Task 1: Implement lock acquisition and lifecycle

**Files:**
- Modify: `src/storage.rs`
- Test: `tests/smoke.rs`

**Interfaces:**
- Produces: `storage::WriterLock::acquire(home: &Path) -> Result<WriterLock>` and RAII cleanup on drop.

- [ ] **Step 1: Write failing lock tests**

Add tests for acquiring a lock, rejecting a second live-PID lock, reclaiming a
dead-PID lock, and rejecting malformed metadata without deleting the file.

- [ ] **Step 2: Run focused tests**

Run: `cargo test writer_lock -- --nocapture`

Expected: FAIL because `WriterLock` does not exist.

- [ ] **Step 3: Implement `WriterLock`**

Use `OpenOptions::create_new(true)` and JSON metadata `{pid, started_at}`.
Treat the current process ID as live; for other PIDs use platform process
existence checks without sending a termination signal. Reclaim only a valid
metadata file whose PID is confirmed dead. Remove the lock file in `Drop`.

- [ ] **Step 4: Run focused and full tests**

Run: `cargo test writer_lock -- --nocapture` and `cargo test`.

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/storage.rs tests/smoke.rs
git commit -m "feat: add single-writer lock"
```

### Task 2: Apply lock to mutating CLI commands

**Files:**
- Modify: `src/main.rs`
- Modify: `tests/smoke.rs`

**Interfaces:**
- Consumes: `storage::WriterLock::acquire` from Task 1.

- [ ] **Step 1: Write failing CLI lock coverage**

Add a test that creates `write.lock`, runs `ingest`, asserts a non-zero exit
with a busy error, and confirms no event was written. Add coverage that
`search` and `stats` still run while the lock exists.

- [ ] **Step 2: Run focused test**

Run: `cargo test cli_respects_writer_lock -- --nocapture`

Expected: FAIL because CLI commands do not acquire/check the lock.

- [ ] **Step 3: Acquire lock in `main::run`**

Acquire after argument parsing and before opening/mutating the store for
`init`, `ingest`, `consolidate`, and `reindex`. Keep `search` and `stats`
lock-free. Hold the guard until `run` returns.

- [ ] **Step 4: Run all gates**

Run:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: all pass.

- [ ] **Step 5: Update docs and commit**

Document lock behavior and stale-lock limitations in `README.md`,
`docs/ARCHITECTURE.md`, and `docs/ROADMAP.md`, then commit:

```bash
git add src/main.rs tests/smoke.rs README.md docs/ARCHITECTURE.md docs/ROADMAP.md docs/superpowers
git commit -m "feat: serialize mutating commands"
```

## Checkpoint

- [ ] Concurrent Plastia writers are serialized.
- [ ] Stale valid locks recover.
- [ ] Invalid locks fail closed.
- [ ] Read-only commands remain available.
- [ ] Formatting, clippy and tests pass.
