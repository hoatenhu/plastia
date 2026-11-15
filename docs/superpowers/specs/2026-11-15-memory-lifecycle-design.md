# Memory Lifecycle and Explicit Conflict Resolution

## Intent

Make durable memories distinguish evidence, active claims, superseded versions,
disputed claims and archived records. Resolve conflicts only through an
explicit CLI action; never infer authority from the free-form `source` field.

## Approach

Extend the SQLite memory model with lifecycle metadata: `status`, `supersedes`,
valid-time fields and provenance fields retained from the source event. Extend
conflicts with resolution state, timestamp and reason. Add bounded CLI commands
to list and resolve pending conflicts.

Resolution is explicit and local:

- `--accept`: the incoming event becomes a new active memory version and the
  existing active memory becomes `superseded`.
- `--reject`: the existing active memory remains active; the incoming claim is
  recorded as rejected evidence and is not promoted.

Both operations append an audit record. No source text, policy label or caller
field grants authority.

## Non-goals

- Authentication or remote multi-user authorization.
- Automatic policy promotion.
- Rewriting or deleting historical events.
- Full validity-time inference.

## Acceptance criteria

- Existing stores migrate without losing memories, conflicts or provenance.
- New memories default to `active` and retain source event attribution.
- Pending conflicts are listable with stable IDs and evidence references.
- Accept creates one active incoming version and supersedes the old memory.
- Reject preserves the old active memory and records the incoming claim as
  rejected without creating an active memory.
- Invalid IDs, non-pending conflicts and missing reasons fail safely.
- Repeating a completed resolution is idempotent and does not create another
  active version.
- All operations have integration tests and pass fmt, clippy and cargo test.
