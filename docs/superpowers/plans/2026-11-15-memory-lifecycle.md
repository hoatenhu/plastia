# Memory Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add versioned memory lifecycle metadata and explicit, auditable conflict resolution.

**Architecture:** Extend existing SQLite tables through additive schema migration. Keep Markdown history and source events intact. Add `conflicts` and `audit_log` CLI operations over the same single-writer/outbox-protected store.

**Tech Stack:** Rust stable, SQLite via `rusqlite`, existing Clap CLI and integration tests.

**Spec:** `docs/superpowers/specs/2026-11-15-memory-lifecycle-design.md`

## Global Constraints

- Never silently overwrite or elevate authority.
- Never trust free-form `source` text as authenticated authority.
- Preserve append-only events, attribution and evidence links.
- Keep conflict resolution explicit and idempotent.
- Use parameterized SQL and validate all CLI inputs.
- No new dependency.

## Review Focus

- Re-resolving an already resolved conflict must not create a second active version.
- A conflict pointing at missing or non-active memory must fail without partial writes.
- Reject must not make the incoming claim searchable as active memory.
- Existing stores with old schemas must migrate without data loss.
- Reasons and IDs must be validated before any transaction commits.

---

### Task 1: Add lifecycle schema and migration

**Files:**
- Modify: `src/storage.rs`
- Test: `tests/smoke.rs`

**Interfaces:**
- Produces additive columns for memory lifecycle, conflict resolution and audit records.

- [ ] Write `memory_lifecycle_schema_migrates_existing_store` first.
- [ ] Run it and verify failure against the old schema.
- [ ] Add idempotent `ALTER TABLE`/table creation migration with defaults:
  `status='active'`, nullable `supersedes`, valid-time fields, conflict
  `resolution='pending'`, and `audit_log`.
- [ ] Run focused and full tests.
- [ ] Commit `feat: add memory lifecycle schema`.

### Task 2: Preserve lifecycle metadata during materialization

**Files:**
- Modify: `src/engine.rs`
- Modify: `src/main.rs`
- Test: `tests/smoke.rs`

**Interfaces:**
- Consumes Task 1 schema; preserves `active` status and provenance on new memory rows.

- [ ] Add failing test asserting consolidated memory has active status, source event, valid metadata and Markdown provenance.
- [ ] Run focused test and observe failure.
- [ ] Update insert/recovery/reindex paths to preserve lifecycle fields and avoid resetting status during reindex.
- [ ] Run focused and full tests.
- [ ] Commit `feat: preserve memory lifecycle metadata`.

### Task 3: Add explicit conflict list and resolution commands

**Files:**
- Modify: `src/engine.rs`
- Modify: `src/main.rs`
- Test: `tests/smoke.rs`

**Interfaces:**
- Produces `list_conflicts(conn, scope)`, `resolve_conflict(conn, conflict_id, decision, reason)`, and CLI commands `conflicts` and `resolve-conflict`.

- [ ] Add failing accept/reject/idempotency integration tests.
- [ ] Run focused tests and observe failure.
- [ ] Implement parameterized list and transactional resolution:
  accept inserts incoming active version, supersedes old memory, marks conflict
  accepted; reject marks conflict rejected and keeps old active memory.
- [ ] Append audit rows for both decisions.
- [ ] Validate conflict ID, decision and non-empty reason before writes.
- [ ] Run focused and full tests.
- [ ] Commit `feat: add explicit conflict resolution`.

### Task 4: Document and verify

**Files:**
- Modify: `README.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/ROADMAP.md`

- [ ] Document lifecycle statuses, CLI examples and explicit-authority limit.
- [ ] Mark only completed roadmap items.
- [ ] Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`.
- [ ] Commit `docs: document memory lifecycle and conflict resolution`.

## Checkpoint

- [ ] Historical events remain append-only.
- [ ] Accept/reject are explicit, transactional and auditable.
- [ ] No conflict can auto-promote policy.
- [ ] Existing stores migrate safely.
- [ ] All verification gates pass.
