# Plastia — Product contract

## One sentence
**Adaptive memory for AI agents:** capture relevant experiences while agents work, recall evidence at the right moment, and progressively reorganize knowledge while agents rest.

## Who it serves
Developers building local-first assistants, coding agents and multi-agent systems using Hermes, Claude Code, Codex, OpenCode, or custom runtimes. Engine stays agent-agnostic, with adapters outside core.

## The problem
A normal vector search system stores and retrieves text, but doesn't reliably distinguish facts from assertions, track changing decisions, consolidate repeated events, or prevent wrong/stale recollection between agents. Existing solutions can be operationally heavy, expensive or opaque. Plastia aims to be useful on small machines with a transparent memory lifecycle.

## User outcomes
1. An opted-in agent conversation automatically leaves a searchable, attributed event trail with no manual `memory_put` call.
2. Explicit durable decisions/preferences become usable soon after observation, subject to authorization; free-form episodes remain available even without LLM.
3. At idle/scheduled times, the engine incrementally deduplicates, links and proposes updates, with replayable evidence, measurable cost, and bounded resource use.
4. Recall prefers relevant, current, verified knowledge over stale assertions and reveals source, confidence category, and validity state.
5. Multiple agents can see consistent shared project memory where explicitly authorized, without leaking private scopes.
6. Users can inspect, correct, export, archive and remove memories and underlying source data under defined retention/deletion policy.

## Memory model
- **Episode**: immutable record of an input/event; untrusted and timestamped.
- **Candidate**: extracted possible fact, decision, preference, outcome or procedure with evidence links.
- **Knowledge**: durable claim with status (`proposed`, `verified`, `superseded`, `disputed`, `archived`), validity, source, and scope.
- **Association**: typed evidence-backed relationship (or explicitly marked hypothesis) between knowledge items.
- **Task state**: must come from canonical task tracker/transactional source, never from approximate recall.

## Operating modes
- **Offline core (default)**: event ingestion, deterministic extraction, FTS5/BM25, scoring, incremental consolidation and audit trail.
- **Optional cognitive adapter**: model-assisted ambiguous extraction, contradiction interpretation and reflections. User must configure provider, budget and disclosure of data transfer; local model option.
- **Automatic integrations**: runtime-specific hooks/adapters; external data capture needs opt-in and exclusions. MCP is a retrieval/control interface, not by itself an event observer.

## Core loop
Capture -> preserve episodes -> deterministic candidate generation -> evaluate provenance/authority -> commit or propose -> recall -> observe outcomes -> idle consolidation -> update evidence graph/validity -> repeat.

## Anti-goals (v0.x)
Human-level cognition, a general agent framework, unsupervised alteration of authenticated policy, mandatory semantic embedding, full graph database, complex UI, always-on LLM, distributed multi-writer sync, remote SaaS, and autonomous execution of remembered instructions.

## Release gates
**P0** durability and correctness; **P1** automatic, consent-based capture with MCP; **P2** incremental scheduled consolidation + safe evolving beliefs; **P3** measurable memory improvement vs no-memory and plain search baselines; **P4** optional model escalation. See roadmap. Names, feature scope and dates are provisional; evidence gates are not.

## North-star measurement
**Evidence-grounded successful task completion with fewer stale-memory errors, acceptable latency and no unauthorized data access.** Stars, number of memory entries and synthetic recall scores are secondary metrics.
