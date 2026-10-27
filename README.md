# Tabguard

A Rust CLI for inspecting CSV datasets and validating them against declarative TOML data-quality contracts.

**Status:** Early development. CSV inspection, schema validation, completeness validation, and column type validation are implemented.

Numeric constraints, categorical membership, uniqueness, cross-column constraints, and advanced profiling are planned.

## Usage

### Inspect a CSV

```bash
task run -- inspect tests/fixtures/customers.csv
```

Reports headers, data-record count, and duplicate headers without printing row values.

### Validate a CSV

```bash
task run -- validate tests/fixtures/typed_valid.csv \
  --contract tests/fixtures/contracts/typed_customers_completeness.toml
```

Example contract:

```toml
version = 1
name = "typed_customers_completeness"

[schema]
required_columns = ["customer_id", "age", "active"]

[completeness]
non_empty_columns = ["age"]

[types.columns]
customer_id = "integer"
age = "number"
active = "boolean"
```

The `completeness` and `types` sections are optional. Their configured columns must also appear in `schema.required_columns`. Additional CSV columns are allowed.

Supported types:

- `integer`: signed 64-bit base-10 integers.
- `number`: finite numeric values.
- `boolean`: `true` or `false`, case-insensitive.

Type checks ignore surrounding whitespace without modifying the original values. Blank and whitespace-only values are excluded from type violations and are governed by configured completeness rules. Type violations are reported as aggregate counts per column, without printing cell values. CSV records are processed as a stream rather than loading the full dataset into memory.

### Exit codes

| Code | Meaning |
|---|---|
| `0` | Validation passed. |
| `1` | Schema or data validation failed, including completeness or type violations. |
| `2` | The contract is invalid or unsupported, or the CSV could not be processed. |

## Development

[Task](https://taskfile.dev/) is the standard interface for routine development commands. Run `task --list` to see all available tasks.

| Command | Purpose |
|---|---|
| `task build` | Build the project. |
| `task check` | Check compilation. |
| `task test` | Run tests using Cargo Nextest. |
| `task fmt` | Format Rust code. |
| `task clippy` | Run Clippy with warnings denied. |
| `task ci` | Run the CI workflow locally using `act`. |
| `task security` | Audit dependencies and scan for secrets. |
| `task release` | Build the optimized release binary. |

## Roadmap

- [x] CSV inspection and structural validation
- [x] Versioned TOML contracts
- [x] Required-column schema validation
- [x] Duplicate-header rejection during schema validation
- [x] Completeness validation
- [x] Column type validation
- [ ] Numeric constraints
- [ ] Categorical membership
- [ ] Value uniqueness
- [ ] Cross-column constraints
- [ ] Advanced profiling and data-distribution analysis
