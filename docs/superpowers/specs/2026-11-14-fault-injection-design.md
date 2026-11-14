# Consolidation Fault-Injection Design

## Intent

Prove that a real Plastia process can stop at write boundaries and that the
next mutating command reclaims the lock, replays the outbox, and leaves one
correct memory.

## Approach

Use an environment-controlled test failpoint. When `PLASTIA_FAILPOINT` equals
a named boundary, the process exits immediately with a non-zero status. The
failpoint is disabled by default and is not part of the public CLI. Integration
tests launch the real binary with the environment variable, inspect the store,
then launch a normal command to verify recovery.

Initial boundary: `after-outbox`, immediately after the outbox intent is
committed and before Markdown materialization. This proves the durable intent
survives process termination and the next command can recover it.

## Acceptance criteria

- Failpoint process exits non-zero at `after-outbox`.
- The outbox intent remains pending and the event is not acknowledged as
  processed.
- The next normal mutating command reclaims the stale lock and recovers exactly
  one Markdown file and memory row.
- A second recovery is idempotent.
- Normal execution remains unchanged when the environment variable is absent.

## Ceiling

This is deterministic process-boundary coverage, not an OS power-loss proof.
Directory `fsync` and broader failpoints remain separate work.
