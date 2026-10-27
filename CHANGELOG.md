# Changelog

All notable changes to **Tabguard** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-11-04

### Added

- CSV inspection with header, record-count, and duplicate-header reporting.
- Versioned TOML contracts for declarative data-quality validation.
- Required-column schema validation and duplicate-header rejection.
- Completeness validation for configured columns.
- Column type validation for signed 64-bit integers, finite numbers, and booleans.
- Numeric constraints with inclusive and exclusive lower and upper bounds.
- Streaming CSV processing without loading the entire dataset into memory.
- Aggregate per-column validation reports without exposing individual cell values.
- Distinct exit codes for successful validation, validation failures, and operational errors.

[Unreleased]: https://github.com/Adam-Al-Rahman/tabguard/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Adam-Al-Rahman/tabguard/releases/tag/v0.1.0
