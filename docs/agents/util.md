# util crate

Role: Small shared utilities.

Contents:
- `CommandRunner`: run external commands with verbosity/dry-run support.
- PATH lookup and permission helpers.
- CAP_NET_ADMIN detection and `is_root` helper.
- `dns` module: resolver parsing and loopback detection.

Tests:
- DNS parsing + loopback checks.
