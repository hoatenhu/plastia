# Plastia — Cognitive memory design (draft)

## Goal

An agent-agnostic memory engine that **learns during activity and consolidates during idle time**. The model is algorithm-first, with optional model-based interpretation where rule-based extraction cannot resolve ambiguity.

## Data lifecycle

1. Capture: tools/hooks stream conversations, documents and outcomes, with stable event IDs and provenance.
2. Episodic buffer: append-only event log, searchable immediately.
3. Online promotion: clear, high-authority or explicitly marked decisions become durable memory quickly.
4. Offline consolidation: process pending events; dedupe, link, classify contradictions and add learned lessons.
5. Recall: score by lexical relevance, recency, scope, confidence and provenance, enforcing active/superseded statuses.
6. Forgetting: reduce retrieval priority for stale, low-value observations; never silently delete audit evidence.

## Deterministic first

- Rules for source credibility, explicit key/value updates, checksums, duplicates and temporal validity.
- Generate candidate associations using shared subjects/scopes and co-occurrence; do not hallucinate semantic links.
- Selectively invoke an optional language model for ambiguous text and contradiction interpretation, within explicit budget.
- Preserve the original event and model decision trace, never promote an inference into a verified policy.

## Trust boundaries

- Source claims may be false or malicious. They are observations, not policy instructions.
- A policy change requires an authenticated authority identity or explicit approval. This MVP cannot authenticate an author, so it records conflicts rather than choosing winners.
- Each agent/project needs scope checks before memories can be read or written across domains.
- A background review may suggest a change but must not overwrite high-authority decisions autonomously.

## Next vertical slice

Priorities: (1) recoverable atomic processing and file locking, (2) explicit conflict resolution/versioning, (3) MCP stdio server, (4) streaming adapter that supplies events automatically, (5) scheduled incremental consolidation.

## Metrics

Track event capture completeness, recall@k, conflict false positives, stale memory rate, tokens per review, indexing time, P95 query time, RSS. Compare no-memory baseline vs raw episodic search vs consolidated memory.
