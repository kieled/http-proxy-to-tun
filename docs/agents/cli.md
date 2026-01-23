# cli crate

Role: Defines CLI flags and subcommands; provides helpers to parse default "up".

Key APIs:
- `parse_cli_with_default_up`: injects `up` if no subcommand given.
- `parse_cli_with_default_up_from`: same logic for tests/explicit args.
- `read_password`: reads inline or file-based password.

Tests:
- Inline and file password reading.
