# Privacy, safety and threat model

Plastia ingests agent conversations and tool/document content, which may contain sensitive or adversarial data. **v0.1 is not suitable for secrets or sensitive personal information.** Default to local/offline operation. Do not treat stored memories as operational instructions.

## Key risks
- Prompt injection: untrusted source content masquerades as policy/system instruction. Stored text remains evidence only; retrieval must be quoted/labelled and never executed.
- Authority spoofing: freeform `source` field is not an authenticated identity. Privileged updates need verifiable principal and ACL check.
- Cross-agent leakage: scope filters are not access control; enforce permissions on read/write operations before ranking.
- Data capture overreach: hooks must be explicitly enabled, session/field exclusions and secret redaction must work before persistence. Document disclosures for remote models.
- Corruption and concurrency: SQLite and Markdown must not diverge permanently; use persisted outbox, recovery and locking.
- Privacy lifecycle: specify retention, export, delete and backup behavior; do not call decaying relevance 'deletion'.
- Resource exhaustion: cap event length, batch size, search results, timeouts, disk growth and model invocation budgets.
- Filesystem attacks: normalize/restrict paths under the store root; defend against traversal/symlinks and malicious filenames.

## Release blocking security gates
No public network binding unauthenticated; no autonomous change to authenticated policy; no sensitive capture by default; no cross-scope read without permissions; no secret in logs; negative security tests in CI.

## Reporting
Before production claims, add `SECURITY.md` at repo root with disclosure contact/process, supported versions and triage instructions. This document is the engineering threat-model baseline, not a full security audit.
