# state crate

Role: Persist runtime state for teardown and debugging (current version: 3).

Key structures:
- `State`: serialized state file (versioned).
- `StateStore`: manages state dir, lock file, state JSON.

Notes:
- `keep_logs` keeps `state.json` and directory; lock file is always removed.

Tests:
- State read/write roundtrip.
- `keep_logs` behavior.
