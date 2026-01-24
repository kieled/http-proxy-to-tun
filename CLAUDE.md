# http-tun - Claude Code Instructions

## Project Overview

Rust tool to easily wrap whole system under HTTP proxy using TUN

**Primary Language**: rust
**Project Type**: desktop

## Architecture

- **Framework**: N/A (Frontend) / N/A (Backend)
- **Package Manager**: bun
- **Testing**: vitest

## Development Commands

### Setup

```bash
# Install dependencies
bun install

# Start development server
bun run dev
```

### Testing

```bash
# Run tests
npx vitest run

# Run linter
npx eslint . && npx prettier --check .
```

### Build

```bash
bun run build
```

## Code Quality Requirements

### General Guidelines

- Write self-documenting code
- Follow existing code patterns
- Keep functions focused and small
- Handle errors appropriately
- No over-engineering - solve the current problem

### Testing Requirements

When user asks to "run tests", execute:
1. Unit tests
2. Build verification
3. Type checking (if applicable)
4. Linting

## Security Requirements

### Mandatory Security Practices

- All user inputs MUST be validated
- Credentials MUST use OS keychain, never plaintext
- SQL queries MUST use parameterized statements
- File operations MUST be sandboxed
- All external data MUST be sanitized

### Security Review Required For

- Authentication/Authorization changes
- Encryption/Cryptography implementations
- External API integrations
- Database schema changes


## Commit Guidelines

Use conventional commit format:
- `feat:` New features
- `fix:` Bug fixes
- `docs:` Documentation
- `refactor:` Code refactoring
- `test:` Adding tests
- `chore:` Maintenance

## Questions?

If unclear about implementation approach, ask for clarification before proceeding.
