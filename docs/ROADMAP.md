# Execution roadmap (coding agent backlog)

**Rule:** Each milestone must land as a reviewable vertical slice, with acceptance tests, docs and measured evidence. Start at M0. Mark completion only when tests actually run. Never skip safety/consent gates for easier demo.

## M0 — Baseline, builds, reproducibility (first task)
- [ ] Build on supported Rust stable and run fmt/clippy/tests on Linux + macOS (Windows optional first pass). Resolve any compile/API issues.
- [ ] Pin a lockfile for reproducible executable builds, CI dependency caching, migration version table, test temp directories.
- [ ] Create CLI end-to-end golden fixtures; tests cover empty/duplicate/malformed events, multiple scopes, dry-run, reindex, unexpected restart.
- [ ] Measure baseline index/search/memory RSS for 1k/10k events (report machine and methodology).
**Exit:** CI green; README commands run; no claim of performance unmeasured.

## M1 — Durability and data integrity
- [ ] Move cross-resource writes to durable, idempotent journal/outbox; atomic memory file replacement with recovery strategy.
- [ ] Add single-writer coordination, transaction boundaries, recovery/retry and schema migrations; repair/rebuild without losing status/history.
- [ ] Explicit schema for provenance, claim status, valid-time and supersedes/version; distinguish duplicate vs independent evidence.
- [ ] Safe conflict resolution command requiring verified authority for policy, plus inspect/audit history.
**Exit:** Kill-process/fault-injection/replay tests demonstrate no permanently lost or silently corrupted acknowledged events; conflicts never auto-override policy.

## M2 — Programmatic API + MCP stdio + automatic capture adapter
- [ ] Introduce core API shared by CLI and MCP. Implement standards-compliant stdio server, clean startup/shutdown and 4 bounded tools.
- [ ] Implement at least one real agent hook adapter (choose a runtime with documented event hooks); opt-in setup, exclusions, pause/resume, stable event IDs.
- [ ] Verify sessions are captured **without manual memory calls**, while secrets/disabled scopes are skipped.
- [ ] Cap response sizes, sanitize metadata, enforce scope permissions and tool arguments.
**Exit:** Integration test captures a sample conversation and tool outcome automatically, survives reconnect/replay, and cannot read a blocked scope. MCP alone is not counted as automatic capture.

## M3 — Automatic incremental consolidation ('dreaming')
- [ ] Idle trigger + optional local scheduler/cron; resumable bounded work queue, dry-run, progress report, throttle, pause/resume.
- [ ] Deterministic extract/normalize/link, duplicate handling, staleness/validity, hypotheses and evidence-based confidence tiers.
- [ ] No silent overwrite of higher-authority knowledge; preserve proposals pending confirmation and source attribution.
- [ ] Evaluation fixtures: correction across sessions, repeated preference, contradictory sources, failed task lesson, multilingual text.
**Exit:** Unattended capture-to-consolidation works with no model/network; deterministic replay yields same materialized state on identical event stream; fails safely on interruption.

## M4 — Better recall and evaluation
- [ ] Introduce explainable ranking incorporating scope, lexical relevance, validity, provenance, and recency (weights measured not invented).
- [ ] Support bounded context assembly and user feedback as observations, not authoritative updates.
- [ ] Compare with raw history FTS and naive latest-only memory on held-out scenarios; prevent data leakage between splits.
**Exit:** Published repeatable scores on grounded tasks, stale-memory false positives, p50/p95 timings and memory/cost, including negative cases. If quality doesn't improve, fix retrieval before adding model complexity.

## M5 — Optional LLM escalation (only if benchmark shows gap)
- [ ] Provider trait, disabled-by-default local model adapter first, optional remote with explicit consent and data redaction.
- [ ] Trigger only on ambiguous or high-value candidates; hard cost/token/time budgets and schema validation.
- [ ] Record model outputs as unverified proposals, link to source, support rollback/inspection.
**Exit:** A/B against deterministic-only with same benchmark, audited cost/error and no automatic policy promotion.

## M6 — Open-source release readiness
- [ ] Cross-platform binaries, docs/examples for at least 2 agents, license notices, security policy, backup/restore and migration instructions.
- [ ] Local privacy controls, data export/delete, retention and sensible config defaults, threat-model review.
- [ ] SemVer release tagging, changelog and reproducible benchmarking instructions.
**Exit:** Fresh install, upgrade, backup/restore, integration and negative authorization tests all pass.

## Explicitly out of scope until after M6
Distributed memory consensus, mandatory vector DB, web dashboard, autonomous model training, unrestricted browser/file monitoring, unsupervised agent action execution and global multi-tenant SaaS.
