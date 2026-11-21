# Contributing to Natrix

Thank you for considering contributing to Natrix! This guide will help you get started with the development process.
Please join the [Discord Server](https://discord.gg/Rptzjf3Nnm) where we can answer any questions you might have! 

>[!WARNING]
> Natrix is a passion project by mainly one dev, it is founded on a very strong idea of ergonomics and idiomatic code.
> As such, especially when it comes to the public api, maintainers might be extremely strict.

## Project Structure

- **crates/natrix/**: Core framework library
- **crates/natrix_macros/**: Procedural macros for state and assets
- **crates/natrix_shared/**: Shared utilities between core and CLI
- **crates/natrix-cli/**: CLI tool for project management
- **ci/**: End-to-end testing, benchmarks, etc
- **docs/**: User guide (mdBook)

## Development Setup

We have a [Nix](https://nixos.org/) flake to setup a working dev environment for natrix.

## Development Workflow

Natrix uses [Serpentine](https://github.com/Serpent-Tools/serpentine) for CI and local testing, the serpentine pipeline is stored in `./ci/snek/`, for easier use you can use the `justfile` targets, 
`fast` runs unit testing and linters, while `ci` runs the slower integration tests as well.

the full pipeline will always be run in CI so dont worry about running the entire thing locally if it takes a while for you.

### Quick Development Iteration

For quick iteration during development, you can also use standard Rust commands like `cargo clippy` and `cargo test`, etc.

## Code Style
Natrix uses a wide range of clippy lints. But we do often use `#[expect]` on certain areas.
**But, this should be done sparingly.**

In most cases make use of `log_or_panic_*` macros to panic on debug builds, but only log error in production. Allowing production panics should only be done in extremely specific cases, effectively only when said panic would also **instantly** be hit in debug builds, for example problems mounting the root element.

Additionally natrix has important invariants in terms of its reactivity system that must not be invalidated.
When implementing new features, try to build on existing functionality in order to minimize the risk of breaking these invariants.

## Claim an issue
* Before working on an issue please ask to be assigned to prevent duplicate work.
* For any issues that touch the public api please discuss the api design in the issue first.

### Labels
* Do not claim issues marked with `maintainer-only`
* If an issue is marked `Status-Needs Discussion`, it (as the name suggests) needs design or implementation discussion, and any proposed implementation should be run by maintainers before being implemented.

## Pull Request Process
1. If possible please try to run test suites before creating a PR.
2. Update documentation if necessary
3. Add tests for new functionality

## AI-Code
Natrix is not against AI-code, but there are rules you must follow:
* We are not interested in llm "drive-by" prs, there are better things to throw your tokens at than github stats. 
* We expect you to take full responsibility for any code you submit.
* Any commit containing AI code must be marked as such using an appropriate `Co-authored-by`/`Assisted-by` trailer in the commit message.
