# Consolidation Fault-Injection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Verify real-process interruption after outbox commit and automatic recovery.

**Architecture:** Add a disabled-by-default environment failpoint at the first durable boundary. Integration tests launch the actual binary, verify pending state after forced exit, then run a normal mutating command to reclaim the lock and replay the outbox.

**Tech Stack:** Rust stable, existing CLI, SQLite, stdlib process APIs.

**Spec:** `docs/superpowers/specs/2026-11-14-fault-injection-design.md`

## Global Constraints

- The failpoint is test-only and disabled unless explicitly configured.
- The outbox remains the recovery source of truth.
- No power-loss guarantee is claimed.
- Existing normal CLI behavior must remain unchanged.

## Review Focus

- Failpoint must not run during normal commands without explicit environment configuration.
- Forced exit must leave a recoverable lock/outbox state.
- Recovery must not duplicate an already materialized memory.
- A failed recovery must not mark the source event processed.

---

### Task 1: Add deterministic process failpoint and recovery test

**Files:**
- Modify: `src/engine.rs`
- Modify: `tests/smoke.rs`

**Interfaces:**
- Produces: internal `failpoint(name: &str)` behavior keyed by `PLASTIA_FAILPOINT`.

- [ ] **Step 1: Write the failing subprocess test**

Add `process_exit_after_outbox_commit_recovers` that initializes a fresh store,
ingests one decision, runs `consolidate` with
`PLASTIA_FAILPOINT=after-outbox`, asserts non-zero exit, then runs normal
`consolidate` twice and asserts one applied outbox row, one memory, one file,
and one processed event.

- [ ] **Step 2: Run the focused test**

Run: `cargo test process_exit_after_outbox_commit_recovers -- --nocapture`

Expected: FAIL because the failpoint does not exist and the first process exits normally.

- [ ] **Step 3: Implement `failpoint(name: &str)`**

Read `PLASTIA_FAILPOINT`; call `std::process::exit(86)` only when it exactly
matches the supplied name. Invoke it after the outbox insert is committed and
before Markdown materialization. Use a short transaction around the intent
insert so the failpoint observes a durable pending row.

- [ ] **Step 4: Run focused and full tests**

Run: `cargo test process_exit_after_outbox_commit_recovers -- --nocapture` and
`cargo test`.

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/engine.rs tests/smoke.rs docs/superpowers
git commit -m "test: verify outbox recovery after process exit"
```

### Task 2: Document the verified boundary

**Files:**
- Modify: `README.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/ROADMAP.md`

- [ ] **Step 1: Document failpoint test scope**

State that the repository verifies process exit after outbox commit and replay,
but does not claim directory `fsync` or power-loss durability.

- [ ] **Step 2: Run all gates**

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Expected: all pass.

- [ ] **Step 3: Commit**

```bash
git add README.md docs/ARCHITECTURE.md docs/ROADMAP.md
git commit -m "docs: record process recovery coverage"
```
