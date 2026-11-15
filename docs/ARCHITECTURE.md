# Architecture and durable contracts

## Boundaries
`agent hooks/adapters -> capture API -> append-only episode store -> candidate extraction -> consolidation planner -> transactional materializer -> search/recall API -> agent adapters`.

Core state and event log live in SQLite. Markdown holds durable human-readable knowledge; SQL holds materialization state and rebuildable FTS5 indexes. Avoid bidirectional ownership ambiguity. Consolidation persists a SQLite outbox intent, applies Markdown through temp-file rename, materializes the SQL row idempotently, and replays pending intents before mutating CLI commands. Mutating CLI commands serialize through `<home>/write.lock`; read-only commands remain lock-free. Directory `fsync` and full power-loss guarantees are not claimed until tested.

## Minimum event envelope (target schema; v0.1 supports fewer fields)
```json
{
  "event_id": "source-stable-id",
  "scope": "project/swich",
  "source_type": "conversation|tool|document|outcome",
  "source_id": "adapter/instance/session",
  "actor_id": "authenticated-principal-or-null",
  "occurred_at": "2026-11-14T07:00:00Z",
  "observed_at": "2026-11-14T07:00:01Z",
  "content": "Text or sanitized structured reference",
  "content_hash": "blake3-hash",
  "permissions": ["memory:write"],
  "privacy": "normal|sensitive|excluded",
  "correlation_id": "session-or-task-id"
}
```
Credentials/permissions must come from verified transport identity, **never** trust these fields if supplied by the caller. Define versioned schemas and migration rules. Preserve source attribution; never conflate time an event occurred with time it was observed.

## Consolidation
- Bounded batches: read pending events by durable cursor/lease; persist attempts, failures and retry state. Guarantee at-least-once processing with idempotent writes, not exactly-once execution promises.
- Deterministic candidate extraction: explicit markers, structured tool outcomes and high-precision patterns; unrecognized text stays episode-only.
- Duplicate handling: normalized fingerprints scoped by source/subject and time; repeat mention can strengthen evidence but must not fabricate independent corroboration.
- Conflicts: create pending resolution with both evidence references; do not overwrite old knowledge or promote incoming claim automatically.
- Association: co-reference/explicit links with traceability; don't imply causation from co-occurrence. Bounded edge count, deterministic scoring.
- Forgetting: decay **retrieval priority**, not audit records. Respect retention/deletion requests separately; privileged policies should not decay into irrelevance.
- Dreaming scheduler: trigger by idle AND/OR nightly, incremental watermark, bounded CPU/budget; run-once, pause, status, dry-run and deterministic replay.
- Optional LLM: route only unresolved candidates, with PII-safe policy, no default network usage, strict response schema, source attribution, independent validation, and quotas. Model suggestions aren't truth.

## Recall contract
Request includes query, caller principal, allowed scope(s), max results/token budget, time context and optional kind. Response includes stable ID, snippet, source references, scope, status, timestamp, evidence type, explanation of relevance. Filter permissions/status **before** ranking. FTS5/BM25 is default; hybrid/embedding behind optional interface only if benchmark warrants.

## API surface
Core Rust API first. CLI for ingest, search, inspect, dry-run/replay, consolidate and stats. Thin MCP stdio implementation with tools `memory_search`, `memory_get`, `memory_feedback`, `memory_status`; write/admin operations must be permission-gated. HTTP optional later, with authentication/TLS or loopback-only default.

## Versioning/upgrade
Version migrations; never rewrite user Markdown without backup or idempotent recovery. Provenance should survive export/import. No silent loss of history during reindex. Keep documented limitations in README until resolved.

## Known v0.1 technical debt
- `consolidate` has a persisted outbox and replay path; CLI writers use a PID-bearing lock, and tests cover process exit after outbox commit. Directory-sync and power-loss coverage remain.
- Memory lifecycle fields preserve `active`, `superseded`, `disputed` and `archived` state, valid-time metadata, supersession and evidence type. Conflict resolution is explicit CLI confirmation with an audit row; it does not authenticate the caller or promote policy automatically.
- `reindex` clears/rebuilds SQL memories but does not preserve durable validity state/versioning, and events are not encoded into Markdown.
- `source` is unauthenticated text. Existing extracted policies are *unverified claims*, not authorized directives.
- Bare-key extraction and FTS OR-token semantics are limited; no domain-aware NLP, bilingual recall tests, or stable ranking across episode/knowledge sources.
- `occurred_at` is text, not validated; derived IDs use current time when absent.
- No consent-aware hook, secrets redaction, encrypted storage or retention/deletion workflow.
- No MCP, background service, model adapter or autonomous consolidation yet.
