# Tabguard

A Rust CLI for inspecting CSV datasets and validating them against declarative data-quality contracts.

Tabguard aims to help developers and data practitioners detect schema mismatches, missing data, invalid values, and other data-quality issues before they affect downstream applications and machine-learning pipelines.

**Status:** Early development. CSV inspection is implemented, and contract-based schema validation is being developed. Validation of individual cell values and advanced data-quality constraints are not yet implemented.

## Development Workflow

Tabguard uses [Task](https://taskfile.dev/) as its standard development command interface.

The `Taskfile.yaml` defines repeatable tasks for building, checking, testing, formatting, linting, running the CLI, and reproducing CI. Use these tasks instead of invoking the underlying tools directly for routine development.

### Available tasks

Run `task --list` to see every available task.

| Task | Purpose |
|---|---|
| `task build` | Build the project. |
| `task check` | Check whether the project compiles. |
| `task test` | Run the complete test suite using Cargo Nextest. |
| `task clippy` | Run Clippy across all targets and treat warnings as errors. |
| `task fmt` | Format the Rust codebase using nightly rustfmt. |
| `task fmt-changed` | Format changed files using the configured Jujutsu fix workflow. |
| `task run -- <arguments>` | Run Tabguard with the supplied CLI arguments. |
| `task debug` | Build the development binary and start the Rust debugger. |
| `task rr` | Build the binary and record an execution using `rr`. |
| `task ci` | Run the pull-request workflow locally using `act`. |
| `task security` | Run dependency auditing and secret scanning. |
| `task release` | Build the optimized release binary. |

### Recommended development cycle

Use the following workflow when implementing a feature:

1. Use Bacon for continuous compilation, testing, and lint feedback while editing.
2. Run `task fmt` to format the code.
3. Run `task check` to verify compilation.
4. Run `task test` to execute the test suite.
5. Run `task clippy` to enforce the project's lint rules.
6. Run `task ci` before publishing changes to check the pull-request workflow locally.

Run `task security` when checking dependencies and secrets.

Bacon is intended for continuous feedback during development; Taskfile provides explicit, repeatable commands. The CI workflow remains the final verification gate.

### Tooling

The project uses:

- [Task](https://taskfile.dev/) for repeatable development commands.
- [Bacon](https://github.com/Canop/bacon) for continuous development feedback.
- [Cargo Nextest](https://nexte.st/) for test execution.
- [Jujutsu](https://jj-vcs.github.io/jj/) for version control.
- [act](https://github.com/nektos/act) for supported local GitHub Actions workflows.

The repository's Cargo configuration uses the Mold linker. Ensure the necessary Rust toolchains and development tools are installed before running the corresponding tasks.

## Usage

### Inspect a CSV dataset

```bash
task run -- inspect tests/fixtures/customers.csv
```

The `inspect` command reports the column headers, number of data records, and duplicate header names. It does not print individual row values or infer column types.

### Validate a CSV dataset against a contract

The Milestone 3 interface is:

```bash
task run -- validate tests/fixtures/customers.csv \
  --contract tests/fixtures/contracts/customers.toml
```

A TOML contract declares the dataset's expected structure:

```toml
version = 1
name = "customers"

[schema]
required_columns = ["customer_id", "email"]
```

The initial schema validator checks whether all declared required columns exist in the CSV header. Additional columns are allowed unless a future contract feature explicitly says otherwise.

Validation distinguishes three outcomes:

| Exit code | Meaning |
|---|---|
| `0` | Validation passed. |
| `1` | The CSV was processed, but one or more required columns are missing. |
| `2` | The CSV or contract could not be processed, or the contract is unsupported. |

These semantics are the intended Milestone 3 behavior; the implementation must pass its tests before this behavior can be considered verified.

## Project Goals

- [ ] Support declarative, version-controlled data-quality contracts.
- [ ] Validate CSV schemas against required-column rules.
- [ ] Extend validation to completeness, types, numeric boundaries, categorical membership, and uniqueness.
- [ ] Generate clear, deterministic validation reports.
- [ ] Provide predictable exit codes for automation and CI.
- [ ] Handle errors explicitly and document memory trade-offs.
- [ ] Support richer profiling and data-distribution comparisons in future milestones.

Tabguard prioritizes understandable contracts, dependable behavior, a maintainable implementation, and explicit validation semantics.
