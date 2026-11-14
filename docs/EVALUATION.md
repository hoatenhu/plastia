# Evidence-based evaluation contract

## Test on real outcomes, not only retrieval demos
Compare A: no memory, B: raw episodic FTS5, C: consolidated memory. Optional D: model-assisted. Use the same scenarios, tool budgets and model configs. Hold out test tasks from extraction prompts/rules.

## Required fixtures
1. Cross-session correction: old production rule, new authenticated rule, old rule must not be recalled as current.
2. Contradiction: untrusted document disagrees with approved policy; do not overwrite.
3. Attribution: PM quotes CEO; quotation does not confer CEO authority.
4. Duplicate replay: same hook event arrives 3x; event count and learning should not triple.
5. Multi-agent: QA sees shared authorized project scope, cannot see unrelated private scope.
6. Recall under time: revoked/stale versus active decisions and outcomes.
7. Security: text such as 'ignore previous instructions' captured as data, not executed or treated as system policy.
8. Privacy: secrets, excluded sessions, deletion and redaction; capture must respect opt-out.
9. Search: short/long queries, typos, non-ASCII Vietnamese and English, null results.
10. Crash/recovery: stop between file creation, SQL materialization and ack; replay preserves correctness.
11. Sleep: rerun same consolidation with unchanged events and no new knowledge; bounded idle CPU.
12. Reflection: a failed action with verified outcome generates evidence-backed lesson, never unsupported fact.

## Metrics
- Capture coverage: eligible events captured / eligible emitted (and privacy false-captures).
- Recall@k and precision@k using human-labeled relevance and evidence.
- Current-fact accuracy, stale-memory error rate, unauthorized disclosure rate (target **zero**).
- Contradiction detection precision/recall; false policy promotion rate (target **zero**).
- Task completion accuracy and latency with/without memory.
- P50/P95 search, ingest and consolidation latency; peak/idle RSS; on-disk bytes; CPU and LLM token/cost budgets.

## Benchmark hygiene
Record test code revision, machine specs, dataset size, cache state, repetitions, seed, representative outputs and failure cases. Avoid comparing raw milliseconds from different machines. Targets are aspirations until measured. Add JSON/CSV benchmark output and a short report; don't report synthetic test results as a production SLA.
