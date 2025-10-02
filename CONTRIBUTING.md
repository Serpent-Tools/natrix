# Contributing to Natrix

Thank you for considering contributing to Natrix! This guide will help you get started with the development process.
Please join the [Discord Server](https://discord.gg/Rptzjf3Nnm) where we can answer any questions you might have! 

## Project Structure

- **crates/natrix/**: Core framework library
- **crates/natrix_macros/**: Procedural macros for state and assets
- **crates/natrix_shared/**: Shared utilities between core and CLI
- **crates/natrix-cli/**: CLI tool for project management
- **ci/**: End-to-end testing, benchmarks, etc
- **docs/**: User guide (mdBook)

## Development Setup

Natrix uses [Dagger](https://dagger.io/) for containerized CI/CD pipeline execution and [`just`](https://github.com/casey/just) for task automation. Please follow the [Dagger installation guide](https://docs.dagger.io/install) to set up your development environment.

We also have a [Nix](https://nixos.org/) flake to setup a working dev environment for natrix.

~~**Why Dagger?** We use Dagger because Natrix has extensive E2E tests and benchmarks that would otherwise require installing around 10 different tools (Chrome, wasm-bindgen, wasm-opt, various Rust tools, etc.). With Dagger, all dependencies are containerized, ensuring consistent builds across environments.~~

> [!NOTE]
> We have since found out dagger doesnt fit our workflow too well, hence we are developing [Serpentine](https://github.com/Serpent-Tools/serpentine) to replace it 

## Development Workflow

### Running the CI Pipeline

The primary way to run tests is through the justfile targets:

- **`just test [jobs]`** - Run the complete test suite 
- **`just test_tui [jobs]`** - Run the complete test suite, but report results to the TUI instead of the allure report.
- **`just book`** - Compile and open the mdbook
- **`just fix`** - Runs automatic fixing of typos, formatting, and outdated snapshot tests.

> [!IMPORTANT]
> * The snapshot tests will assume the new results are correct, always verify the new output is correct in the error logs first.
> * `typos` might assume the wrong correction, always inspect the suggestions in its errors before running `just fix`


### Quick Development Iteration

For quick iteration during development, you can also use standard Rust commands like `cargo clippy` and `cargo test`, etc.

## Code Style
Natrix uses a wide range of clippy lints. But we do often use `#[expect]` on certain areas.
**But, this should be done sparingly.**

In most cases make use of `log_or_panic_*` macros to panic on debug builds, but only log error in production. Allowing production panics should only be done in extremely specific cases, effectively only when said panic would also **instantly** be hit in debug builds, for example problems mounting the root element.

Additionally natrix has important invariants in terms of its reactivity system that must not be invalidated.
When implementing new features, try to build on existing functionality in order to minimize the risk of breaking these invariants.

## Claim a issue
* Before working on a issue please ask to be assigned to prevent duplicate work.
* For any issues that touch the public api please discuss the api design in the issue first.

## Pull Request Process
1. If possible please try to run test suits before creating a PR.
2. Update documentation if necessary
3. Add tests for new functionality
