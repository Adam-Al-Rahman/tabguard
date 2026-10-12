# Tabguard

A Rust CLI for validating structured datasets against declarative data-quality contracts.

Tabguard aims to help developers and data practitioners detect invalid values, missing data, schema mismatches, and other data-quality issues before they affect downstream applications and machine-learning pipelines.

**Status:** Early development. Data-validation functionality has not yet been implemented.

## Development Workflow

Tabguard uses the following tools to keep development and verification consistent:

* [Task](https://taskfile.dev/) — Run repeatable development commands defined in `Taskfile.yaml`.
* [Bacon](https://github.com/Canop/bacon) — Get continuous compilation, linting, and testing feedback while editing.
* [act](https://github.com/nektos/act) — Run supported GitHub Actions workflows locally to check CI behavior before pushing changes.

Use Task for explicit project tasks and Bacon for continuous feedback during development. The `ci` Taskfile task uses `act` to run the pull-request workflow locally.

Run `task --list` to inspect the available tasks.

## Project Goals

* [ ] Support declarative, version-controlled data-quality contracts.
* [ ] Validate CSV datasets against defined data-quality rules.
* [ ] Generate clear, structured validation reports.
* [ ] Provide predictable behavior for automation and CI.
* [ ] Handle errors explicitly and document memory trade-offs.
