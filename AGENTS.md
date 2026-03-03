# Repository Guidelines

## Project Structure & Module Organization
This repository is a Rust workspace for the DeepBrainChain node/runtime stack.
- `node/`: binary entrypoints, CLI, service wiring, RPC bootstrapping.
- `runtime/`: `dbc-runtime` and precompiles; runtime feature flags live here.
- `pallets/`: domain pallets (for example `task-mode`, `agent-attestation`, `x402-settlement`) and related RPC crates.
- `primitives/` and `client/`: shared types and client-side tracing/RPC support.
- `tests/integration/`: Rust integration tests across pallets.
- `extrinsic-test/`: JS scripts for chain/extrinsic interaction checks.
- `scripts/`, `chain-specs/`, `docs/`: utilities, chain specs, and operator docs.

## Build, Test, and Development Commands
Prefer `make` targets (mirrors CI):
- `make build`: build full workspace in release mode.
- `make build-runtime`: build `dbc-runtime` WASM artifact.
- `make run`: run a local dev node with EVM tracing and RPC enabled.
- `make test`: run all tests (`cargo nextest run` if available, else `cargo test --all`).
- `make fmt`: format all crates with pinned nightly rustfmt.

Direct checks used by CI:
- `cargo +nightly-2023-09-20 fmt --all -- --check`
- `cargo check --features try-runtime`
- `cargo check --features runtime-benchmarks`

## Coding Style & Naming Conventions
- Rust 2021 edition, max line width 100 (`rustfmt.toml`).
- Use `snake_case` for modules/functions/files, `CamelCase` for types/traits, `SCREAMING_SNAKE_CASE` for constants.
- Keep pallet APIs and runtime config traits explicit; avoid wildcard imports in new code.
- Format before opening a PR; keep generated artifacts and weights in dedicated commits when possible.

## Testing Guidelines
- Unit tests typically live in each crate (`src/tests.rs` or `mod tests` in `lib.rs`).
- Integration scenarios belong in `tests/integration`.
- Run targeted tests while iterating (example: `cargo test -p pallet-task-mode`).
- Ensure feature-sensitive changes compile and test with `try-runtime` and `runtime-benchmarks` features.

## Commit & Pull Request Guidelines
- Follow Conventional Commit style seen in history: `feat(...)`, `fix(...)`, `chore(...)`, `test(...)`.
- Keep commit subjects imperative and scoped (example: `fix(staking): route Slash to Treasury`).
- PRs should include:
  - concise problem/solution summary,
  - linked issue (if available),
  - test evidence (commands run),
  - runtime/WASM or benchmark impact notes when relevant.
- If changing RPC/runtime behavior, include example calls or payload changes (`rpc.http`, `dbc_rpc.json`, `dbc_types.json`).
