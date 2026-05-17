# Contributing

## Branches

- `feat/PBI-XXX-description` for new features
- `fix/description` for bug fixes
- `chore/description` for tooling/docs

## Commit messages

Use a prefix: `feat:`, `fix:`, `chore:`, `test:`, `docs:`

## Pull requests

One PBI per PR. Include the PBI number in the title (e.g. `feat(PBI-001): scan report`).
Open an Issue before starting work on a new feature.

## Quality gates

Every PR must pass:
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`
