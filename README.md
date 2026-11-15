# Plastia (experimental v0.1.0)

**Local-first, LLM-optional memory engine for AI agents — written in Rust.**

> Prototype, **not production ready**. This repository is a working *core-engine scaffold*. It does not yet include MCP, agent adapters, background watchers, or autonomous dreaming. "Automatic" in this version means event batches are indexed and deterministic memory candidates are extracted automatically when `consolidate` is run; capturing events from live agent conversations still requires an external adapter/hook.

## Approach

- **Episodic memory:** incoming conversation/research events are recorded in SQLite and can be searched immediately with FTS5.
- **Semantic memory:** explicit durable statements marked `Remember:`, `Decision:`, `Policy:`, `Lesson:`, or `Preference:` become Markdown files when consolidated.
- **Offline consolidation:** `consolidate` consumes new events incrementally, deduplicates and records conflicting key/value facts without overwriting the previous decision.
- **No model required:** extraction is intentionally conservative. Unstructured conversations remain searchable as episodes, but are **not yet fully understood** or semantically summarized.
- **Provenance:** each memory points to its originating event. Markdown durable memories can rebuild their SQLite index.

## Build

Requires a recent stable Rust toolchain. SQLite is bundled through `rusqlite`.

```sh
cargo build --release
cargo test
cargo run -- init
cargo run -- ingest examples/conversation.json
cargo run -- search "production approval" --scope project/swich
cargo run -- consolidate --dry-run
cargo run -- consolidate
cargo run -- search "CEO"
cargo run -- stats
cargo run -- reindex
```

Default home: `.plastia`. Override it with `--home /path/to/store` (before or after subcommands; global option).

## Ingest format

JSON array, one event object, or `{ "events": [ ... ] }`:

```json
{
  "events": [
    {
      "id": "source-stable-unique-event-id",
      "scope": "project/swich",
      "source": "hermes/pm",
      "occurred_at": "2026-11-14T09:00:00Z",
      "text": "Policy: production-approval = CEO approval required"
    }
  ]
}
```

- `text` required. `id`, `scope`, `source`, `occurred_at` optional; stable source-provided IDs recommended for replay idempotency. If omitted, IDs are derived from the event contents and current time.
- Timestamp strings are currently stored as text without parsing. Prefer UTC ISO-8601.
- `scope` is an opaque namespace; use it to isolate projects. There is not yet authorization or multi-user isolation.
- **Warning:** do not ingest secrets, API keys, medical records or other sensitive content. No encryption, retention policy, redaction or permission system exists yet.

## Automatic extraction (v0.1)

Rules recognize one durable statement per line:

```text
Decision: staging-gate = QA must attach actual black-box evidence
Policy: prod-approval = CEO must approve production
Lesson: smoke-tests = Always validate release before sync
Remember: docs-location = Docs live in /docs
Preference: reports = Show BLUF first
```

When a record with the same `scope + kind + key` already exists:
- identical value: skip (dedup);
- different value: add a **pending conflict**; the old active memory remains untouched.

This is a deliberate safety-first design, **not** natural language memory extraction. Full automatic understanding and beliefs with uncertainty are future work.

## Storage

```text
.plastia/
  memory.sqlite3        # event log, conflict registry and FTS5 index
  memories/
    <id>.md             # durable memory; JSON metadata in HTML comment
```

Use `plastia reindex` to rebuild the *durable memory* SQL index from Markdown (events remain in SQLite). Do not edit `memory.sqlite3` manually. Consolidation records Markdown memory writes in a SQLite outbox and replays pending intents before mutating commands. Mutating commands acquire `.plastia/write.lock`; live writers are rejected and stale locks with dead PIDs can be reclaimed. If a target file already exists with different content, recovery fails without overwriting it. Directory `fsync` power-loss guarantees are not yet claimed.

## Roadmap

- [x] Event ingestion, idempotent event IDs, FTS5 event search
- [x] Incremental deterministic consolidation, dry run
- [x] Markdown durable memories, full-text search, reindex
- [x] Conflict detection and provenance
- [x] Idempotent consolidation outbox and replay recovery
- [x] Single-writer lock for mutating commands
- [x] Versioned memory lifecycle and explicit conflict resolution
- [ ] Stronger crash durability and directory `fsync`
- [ ] Conflict resolution, confidence/authority policies, version history
- [ ] Hook adapters: Hermes, Claude Code, Codex, OpenCode
- [ ] MCP server (stdio first)
- [ ] Incremental background scheduler and idle/nightly reviews
- [ ] Language-aware heuristic extraction and optional LLM escalation
- [ ] Forgetting policy, association graph, evaluation harness
- [ ] Security: credentials redaction, encryption, retention and ACLs

See [DESIGN.md](DESIGN.md) for the intended cognitive lifecycle.

## Tests

The end-to-end smoke tests check capture, search, dry-run semantics, durable memory creation, outbox replay, conflicting-file protection, conflict registration, lifecycle-preserving reindex, explicit accept/reject resolution and real-process recovery after the `after-outbox` failpoint. CI checks Rust formatting, clippy and tests. Directory `fsync` and power-loss durability remain future work.

Conflicts remain pending until explicitly resolved. List them with `plastia conflicts`, then use `plastia resolve-conflict <id> --accept --reason "..."` or `--reject`. This local v0.x command is explicit confirmation, not authenticated multi-user authorization; the free-form event `source` field never grants authority.

## License

MIT OR Apache-2.0 (dual licensed). See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).

## For coding agents and contributors

Start with **[AGENTS.md](AGENTS.md)** (execution rules), **[PRODUCT.md](PRODUCT.md)** (vision and non-goals), **[docs/ROADMAP.md](docs/ROADMAP.md)** (milestone order and exit criteria), **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** (contracts and known gaps), **[docs/EVALUATION.md](docs/EVALUATION.md)** (benchmark scenarios), and **[docs/SECURITY.md](docs/SECURITY.md)** (threat model). A ready-to-paste coding agent instruction is in [examples/AGENT_START_PROMPT.md](examples/AGENT_START_PROMPT.md).

**Do not treat roadmap/checklist items as implemented.** The initial source has not been cargo-compiled in this environment. First milestone is build verification and durability groundwork.
