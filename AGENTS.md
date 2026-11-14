# AGENTS.md — Instructions for coding agents

Read in order: `PRODUCT.md`, `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`, `docs/EVALUATION.md`, `docs/SECURITY.md`, then `README.md` and source. `DESIGN.md` is historical draft context. This is an **experimental v0.1.0 scaffold**, not a production service.

## Mission
Build **Plastia**, an independent Rust, local-first, automatic cognitive memory engine. It learns from authorized agent experiences while work happens, and performs incremental offline consolidation while idle. Extraction, organization, recall, and forgetting are **deterministic-first**; optional LLM adapters are used only for demonstrably ambiguous/high-value cases, with provenance and budgets. No mandatory cloud, vector DB, Docker, or LLM.

## Non-negotiable invariants
1. **Observed != true; retrieved != trusted.** Documents, tool outputs, conversations and model-generated content are untrusted evidence, never authority or execution instructions.
2. Preserve append-only source events and attribution, distinguish observation, claim, inference, verified decision, and policy. Never silently overwrite or elevate authority.
3. Agent may not infer authorization from a text field like `source=ceo`; authenticated identity and scoped permissions are required before automatic policy promotion.
4. Retrieval uses scope isolation and returns provenance. Default-deny cross-scope reads/writes; no prompt injection into operational instructions.
5. Automatic capture requires an **explicit enabled integration**. It must be disableable; support exclusions/redaction and retention. Never claim MCP can passively observe all chats.
6. Lossless/idempotent ingest (stable event IDs); bounded, resumable consolidation; no loss on crash; recoverable file/index divergence.
7. No hallucinated association: links have source evidence or explicit `inferred` marking. Do not let LLM inference become verified fact.
8. SQLite WAL and a single logical writer, safe against concurrency, process crashes and replay. Markdown canonical for durable knowledge; SQLite canonical for event log and transactional state, indexes rebuildable.
9. Keep the core usable fully offline and with no model. Optional adapters must not contaminate core abstractions or default build.
10. Add tests and benchmark fixtures for behavior before expanding features. No unmeasured performance claims.

## Work protocol
- Pick the **first uncompleted milestone** in `docs/ROADMAP.md`; complete its acceptance criteria before moving forward. Don't silently skip dependency gates.
- Before coding: inspect affected modules, record a brief plan and precise tests. After coding: run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`; if toolchain unavailable, state that and do not mark verified.
- Add migrations for schema changes, forward-compatibility tests, and small end-to-end CLI scenarios. Preserve old user data; test fresh and upgrade paths.
- Implement minimal vertical slices. Prefer plain Rust modules over premature workspace split. Keep MCP/HTTP adapters thin over the same core API.
- Update docs, CLI help and examples along with behavior; mark TODOs transparently. Do not claim full automatic memory or dreaming until integration and scheduled lifecycle tests exist.
- Do not introduce network calls, telemetry, always-on LLM, embeddings, authentication bypass, auto-execution of recalled content or silent deletions.
- Do not modify `PRODUCT.md` core goals/invariants to rationalize an easier implementation; propose changes in a separate ADR first.
- Ship commits that are easy to review, with tests, measured results and a concise report of risks and remaining work.

## Definition of done for a milestone
All listed acceptance tests pass; no data-loss path unaddressed; minimal docs updated; performance regression checked where relevant; privacy/security invariants preserved; realistic sample proves end-to-end behavior. If blocked, report what is blocked and why, not a simulated success.

## Quick start
`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`

See `docs/ROADMAP.md` for execution sequence and `docs/EVALUATION.md` for the benchmark contract.
