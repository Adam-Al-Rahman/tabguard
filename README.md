# Tabguard

A Rust CLI for inspecting CSV datasets and validating them against declarative TOML data-quality contracts.

**Status:** Early development. The CLI and contract format may evolve before the 1.0 release.

## Features

- **CSV inspection** — Report headers, data-record counts, and duplicate headers without exposing row values.
- **Schema validation** — Enforce required columns and reject duplicate headers.
- **Completeness validation** — Detect blank values in configured columns.
- **Column type validation** — Validate signed 64-bit integers, finite numbers, and booleans.
- **Numeric constraints** — Enforce inclusive and exclusive numeric boundaries.
- **Streaming processing** — Process CSV records without loading the entire dataset into memory.
- **Aggregate reporting** — Report type and numeric violations per column without exposing individual cell values.

## Installation

Prebuilt binaries and platform-specific installers are distributed through [GitHub Releases](https://github.com/Adam-Al-Rahman/tabguard/releases).

The installer commands below will become available when the first release is published.

### Linux and macOS

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/Adam-Al-Rahman/tabguard/releases/latest/download/tabguard-installer.sh | sh
```

### Windows

Run in PowerShell:

```powershell
irm https://github.com/Adam-Al-Rahman/tabguard/releases/latest/download/tabguard-installer.ps1 | iex
```

The installers use the Cargo home binary directory. Ensure that directory is included in your `PATH`.

Verify the installation:

```bash
tabguard --version
```

### Build from source

Requires the Rust toolchain.

```bash
git clone https://github.com/Adam-Al-Rahman/tabguard.git
cd tabguard
cargo install --path .
```

## Quick start

Create a CSV file named `customers.csv`:

```csv
customer_id,age,active
1001,32,true
1002,27,false
1003,45,true
```

Inspect the dataset:

```bash
tabguard inspect customers.csv
```

Validate it against a contract:

```bash
tabguard validate customers.csv --contract contracts/customers.toml
```

## Contract configuration

Tabguard uses versioned TOML contracts to describe the expected structure and constraints of a CSV dataset.

Example contract (`contracts/customers.toml`):

```toml
version = 1
name = "customers"

[schema]
required_columns = ["customer_id", "age", "active"]

[completeness]
non_empty_columns = ["age"]

[types.columns]
customer_id = "integer"
age = "number"
active = "boolean"

[numeric_constraints.columns.age]
min = 18
max = 120
```

### Supported types

| Type | Accepted values |
|---|---|
| `integer` | Signed 64-bit base-10 integers |
| `number` | Finite numeric values |
| `boolean` | `true` or `false`, case-insensitive |

Type checks ignore surrounding whitespace without modifying the original values.

### Numeric constraints

Numeric constraints require a declared `integer` or `number` column type.

| Property | Boundary |
|---|---|
| `min` | Inclusive lower bound |
| `max` | Inclusive upper bound |
| `min_exclusive` | Exclusive lower bound |
| `max_exclusive` | Exclusive upper bound |

Each column supports at most one lower-bound property and one upper-bound property. Integer columns require integer bounds; number columns support integer and floating-point bounds.

### Validation rules

- `[completeness]`, `[types.columns]`, and `[numeric_constraints.columns]` are optional.
- Columns configured in these sections must also appear in `schema.required_columns`.
- Additional CSV columns are allowed.
- Blank and whitespace-only values are excluded from type and numeric checks and are handled by completeness rules.
- Values failing type validation are not counted again as numeric constraint violations.
- Invalid bounds, incompatible types, conflicting boundaries, and empty or impossible intervals are rejected as invalid contracts.
- CSV records are processed as a stream.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Validation passed. |
| `1` | Schema or data validation failed. |
| `2` | Invalid or unsupported contract, or CSV processing error. |

## Development

[Task](https://taskfile.dev/) provides the standard interface for local development commands.

List available tasks:

```bash
task --list
```

Clone the repository and build the project:

```bash
git clone https://github.com/Adam-Al-Rahman/tabguard.git
cd tabguard
task build
```

| Command | Description |
|---|---|
| `task build` | Build the project. |
| `task check` | Check compilation. |
| `task test` | Run tests with Cargo Nextest. |
| `task fmt` | Format Rust code. |
| `task clippy` | Run Clippy with warnings denied. |
| `task ci` | Run the CI workflow locally with `act`. |
| `task security` | Audit dependencies and scan for secrets. |
| `task release` | Build the optimized release binary. |

### Run from source

Inspect a CSV:

```bash
task run -- inspect tests/fixtures/customers.csv
```

Validate a CSV:

```bash
task run -- validate tests/fixtures/typed_valid.csv \
  --contract tests/fixtures/contracts/typed_customers_completeness.toml
```

## License

Tabguard is distributed under the [MIT License](LICENSE).
