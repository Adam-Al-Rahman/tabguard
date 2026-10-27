use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashSet},
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

const SUPPORTED_CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct SchemaContract {
    pub(crate) required_columns: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompletenessContract {
    pub(crate) non_empty_columns: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ColumnType {
    Integer,
    Number,
    Boolean,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypesContract {
    pub(crate) columns: BTreeMap<String, ColumnType>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(untagged)]
pub(crate) enum NumericLiteral {
    Integer(i64),
    Float(f64),
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct NumericConstraint {
    pub(crate) min: Option<NumericLiteral>,
    pub(crate) max: Option<NumericLiteral>,
    pub(crate) min_exclusive: Option<NumericLiteral>,
    pub(crate) max_exclusive: Option<NumericLiteral>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct NumericConstraintsContract {
    pub(crate) columns: BTreeMap<String, NumericConstraint>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Contract {
    pub(crate) version: u32,
    pub(crate) name: String,
    pub(crate) schema: SchemaContract,

    #[serde(default)]
    pub(crate) completeness: Option<CompletenessContract>,

    #[serde(default)]
    pub(crate) types: Option<TypesContract>,

    #[serde(default)]
    pub(crate) numeric_constraints: Option<NumericConstraintsContract>,
}

#[derive(Debug)]
pub(crate) enum ContractError {
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseToml {
        path: PathBuf,
        source: toml::de::Error,
    },
    UnsupportedVersion {
        path: PathBuf,
        version: u32,
    },
    DuplicateRequiredColumn {
        path: PathBuf,
        column: String,
    },
    DuplicateNonEmptyColumn {
        path: PathBuf,
        column: String,
    },
    CompletenessColumnNotRequired {
        path: PathBuf,
        column: String,
    },
    TypeColumnNotRequired {
        path: PathBuf,
        column: String,
    },
    InvalidNumericConstraint {
        path: PathBuf,
        column: String,
        reason: String,
    },
    NumericConstraintColumnNotRequired {
        path: PathBuf,
        column: String,
    },
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadFile { path, source } => {
                write!(
                    f,
                    "could not read contract '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::ParseToml { path, source } => {
                write!(
                    f,
                    "could not parse contract '{}': {}",
                    path.display(),
                    source
                )
            }
            Self::UnsupportedVersion { path, version } => {
                write!(
                    f,
                    "unsupported contract version {version} in '{}';\
            supported version is {SUPPORTED_CONTRACT_VERSION}",
                    path.display()
                )
            }

            Self::DuplicateRequiredColumn { path, column } => {
                write!(
                    f,
                    "duplicate required column '{column}' in contract '{}'",
                    path.display()
                )
            }
            Self::DuplicateNonEmptyColumn { path, column } => {
                write!(
                    f,
                    "duplicate non-empty column '{column}' in contract '{}'",
                    path.display()
                )
            }
            Self::CompletenessColumnNotRequired { path, column } => {
                write!(
                    f,
                    "completeness column '{column}' must also appear in schema.required_columns in contract '{}'",
                    path.display()
                )
            }

            Self::TypeColumnNotRequired { path, column } => {
                write!(
                    f,
                    "type column '{column}' must also appear in schema.required_columns in contract '{}'",
                    path.display()
                )
            }

            Self::InvalidNumericConstraint {
                path,
                column,
                reason,
            } => {
                write!(
                    f,
                    "invalid numeric constraint for column '{column}' in contract '{}': {reason}",
                    path.display()
                )
            }

            Self::NumericConstraintColumnNotRequired { path, column } => {
                write!(
                    f,
                    "numeric constraint column '{column}' must also appear in schema.required_columns in contract '{}'",
                    path.display()
                )
            }
        }
    }
}

impl Error for ContractError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ReadFile { source, .. } => Some(source),
            Self::ParseToml { source, .. } => Some(source),
            Self::UnsupportedVersion { .. }
            | Self::DuplicateRequiredColumn { .. }
            | Self::DuplicateNonEmptyColumn { .. }
            | Self::CompletenessColumnNotRequired { .. }
            | Self::TypeColumnNotRequired { .. }
            | Self::InvalidNumericConstraint { .. }
            | Self::NumericConstraintColumnNotRequired { .. } => None,
        }
    }
}

fn invalid_numeric_constraint(path: &Path, column: &str, reason: &str) -> ContractError {
    ContractError::InvalidNumericConstraint {
        path: path.to_owned(),
        column: column.to_owned(),
        reason: reason.to_owned(),
    }
}

fn compare_float_to_integer(value: f64, integer: i64) -> Ordering {
    // i64::MIN is exactly representable as f64.
    let min_i64 = i64::MIN as f64;

    // This is 2^63, one greater than i64::MAX.
    // It is also exactly representable as f64.
    let max_i64_exclusive = -min_i64;

    if value < min_i64 {
        return Ordering::Less;
    }

    if value >= max_i64_exclusive {
        return Ordering::Greater;
    }

    let truncated = value.trunc() as i64;

    match truncated.cmp(&integer) {
        Ordering::Less => Ordering::Less,
        Ordering::Greater => Ordering::Greater,
        Ordering::Equal => match value.fract() {
            fraction if fraction < 0.0 => Ordering::Less,
            fraction if fraction > 0.0 => Ordering::Greater,
            _ => Ordering::Equal,
        },
    }
}

fn compare_float_to_numeric(value: f64, bound: NumericLiteral) -> Ordering {
    match bound {
        NumericLiteral::Integer(integer) => compare_float_to_integer(value, integer),
        NumericLiteral::Float(float) => value
            .partial_cmp(&float)
            .expect("numeric bounds must be finite"),
    }
}

fn next_up_finite(value: f64) -> Option<f64> {
    if !value.is_finite() {
        return None;
    }

    if value == 0.0 {
        return Some(f64::from_bits(1));
    }

    let bits = value.to_bits();

    let next_bits = if value.is_sign_positive() {
        bits.checked_add(1)?
    } else {
        bits.checked_sub(1)?
    };

    let next = f64::from_bits(next_bits);

    next.is_finite().then_some(next)
}

fn first_f64_at_or_above(bound: NumericLiteral) -> Option<f64> {
    let candidate = match bound {
        NumericLiteral::Integer(integer) => integer as f64,
        NumericLiteral::Float(float) => float,
    };

    if !candidate.is_finite() {
        return None;
    }

    match compare_float_to_numeric(candidate, bound) {
        Ordering::Less => next_up_finite(candidate),
        Ordering::Equal | Ordering::Greater => Some(candidate),
    }
}

fn integer_interval_is_non_empty(
    lower: Option<(NumericLiteral, bool)>,
    upper: Option<(NumericLiteral, bool)>,
) -> bool {
    let effective_lower = match lower {
        Some((NumericLiteral::Integer(value), exclusive)) => {
            value as i128 + if exclusive { 1 } else { 0 }
        }
        None => i64::MIN as i128,
        Some((NumericLiteral::Float(_), _)) => {
            unreachable!("integer column bounds must be integer literals")
        }
    };

    let effective_upper = match upper {
        Some((NumericLiteral::Integer(value), exclusive)) => {
            value as i128 - if exclusive { 1 } else { 0 }
        }
        None => i64::MAX as i128,
        Some((NumericLiteral::Float(_), _)) => {
            unreachable!("integer column bounds must be integer literals")
        }
    };

    effective_lower <= effective_upper
}

fn number_interval_is_non_empty(
    lower: Option<(NumericLiteral, bool)>,
    upper: Option<(NumericLiteral, bool)>,
) -> bool {
    let first_allowed = match lower {
        None => -f64::MAX,

        Some((bound, exclusive)) => {
            let Some(candidate) = first_f64_at_or_above(bound) else {
                return false;
            };

            if exclusive && compare_float_to_numeric(candidate, bound) != Ordering::Greater {
                let Some(next) = next_up_finite(candidate) else {
                    return false;
                };

                next
            } else {
                candidate
            }
        }
    };

    let Some((upper_bound, upper_exclusive)) = upper else {
        return true;
    };

    match compare_float_to_numeric(first_allowed, upper_bound) {
        Ordering::Less => true,
        Ordering::Equal => !upper_exclusive,
        Ordering::Greater => false,
    }
}

fn validate_numeric_constraint(
    path: &Path,
    column: &str,
    constraint: NumericConstraint,
    column_type: ColumnType,
) -> Result<(), ContractError> {
    let invalid = |reason| invalid_numeric_constraint(path, column, reason);

    if constraint.min.is_none()
        && constraint.max.is_none()
        && constraint.min_exclusive.is_none()
        && constraint.max_exclusive.is_none()
    {
        return Err(invalid("at least one numeric bound is required"));
    }

    if constraint.min.is_some() && constraint.min_exclusive.is_some() {
        return Err(invalid("specify either min or min_exclusive, not both"));
    }

    if constraint.max.is_some() && constraint.max_exclusive.is_some() {
        return Err(invalid("specify either max or max_exclusive, not both"));
    }

    if column_type == ColumnType::Boolean {
        return Err(invalid(
            "numeric constraints cannot be applied to boolean columns",
        ));
    }

    let bounds = [
        constraint.min,
        constraint.max,
        constraint.min_exclusive,
        constraint.max_exclusive,
    ];

    if bounds
        .into_iter()
        .flatten()
        .any(|bound| matches!(bound, NumericLiteral::Float(value) if !value.is_finite()))
    {
        return Err(invalid("numeric bounds must be finite"));
    }

    if column_type == ColumnType::Integer
        && bounds
            .into_iter()
            .flatten()
            .any(|bound| matches!(bound, NumericLiteral::Float(_)))
    {
        return Err(invalid("integer columns require integer numeric bounds"));
    }

    let lower = constraint
        .min
        .map(|bound| (bound, false))
        .or_else(|| constraint.min_exclusive.map(|bound| (bound, true)));

    let upper = constraint
        .max
        .map(|bound| (bound, false))
        .or_else(|| constraint.max_exclusive.map(|bound| (bound, true)));

    let interval_is_valid = match column_type {
        ColumnType::Integer => integer_interval_is_non_empty(lower, upper),
        ColumnType::Number => number_interval_is_non_empty(lower, upper),
        ColumnType::Boolean => unreachable!("boolean columns were rejected above"),
    };

    if !interval_is_valid {
        return Err(invalid(
            "numeric bounds define an empty or impossible interval",
        ));
    }

    Ok(())
}

pub(crate) fn load_contract(path: &Path) -> Result<Contract, ContractError> {
    let contents = fs::read_to_string(path).map_err(|source| ContractError::ReadFile {
        path: path.to_owned(),
        source,
    })?;

    let contract: Contract =
        toml::from_str(&contents).map_err(|source| ContractError::ParseToml {
            path: path.to_owned(),
            source,
        })?;

    if contract.version != SUPPORTED_CONTRACT_VERSION {
        return Err(ContractError::UnsupportedVersion {
            path: path.to_owned(),
            version: contract.version,
        });
    };

    let mut required_columns = HashSet::with_capacity(contract.schema.required_columns.len());

    for column in &contract.schema.required_columns {
        if !required_columns.insert(column.as_str()) {
            return Err(ContractError::DuplicateRequiredColumn {
                path: path.to_owned(),
                column: column.clone(),
            });
        }
    }

    if let Some(completeness) = &contract.completeness {
        let mut seen_columns = HashSet::with_capacity(completeness.non_empty_columns.len());

        for column in &completeness.non_empty_columns {
            if !seen_columns.insert(column.as_str()) {
                return Err(ContractError::DuplicateNonEmptyColumn {
                    path: path.to_owned(),
                    column: column.clone(),
                });
            }

            if !required_columns.contains(column.as_str()) {
                return Err(ContractError::CompletenessColumnNotRequired {
                    path: path.to_owned(),
                    column: column.clone(),
                });
            }
        }
    }

    if let Some(numeric_constraints) = &contract.numeric_constraints {
        for (column, constraint) in &numeric_constraints.columns {
            if !required_columns.contains(column.as_str()) {
                return Err(ContractError::NumericConstraintColumnNotRequired {
                    path: path.to_owned(),
                    column: column.clone(),
                });
            }

            let column_type = contract
                .types
                .as_ref()
                .and_then(|types| types.columns.get(column))
                .copied()
                .ok_or_else(|| {
                    invalid_numeric_constraint(
                        path,
                        column,
                        "numeric constraints require a declared integer or number column type",
                    )
                })?;

            validate_numeric_constraint(path, column, *constraint, column_type)?;
        }
    }

    if let Some(types) = &contract.types {
        for column in types.columns.keys() {
            if !required_columns.contains(column.as_str()) {
                return Err(ContractError::TypeColumnNotRequired {
                    path: path.to_owned(),
                    column: column.clone(),
                });
            }
        }
    }
    Ok(contract)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{ColumnType, ContractError, NumericLiteral, load_contract};

    fn contract_fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/contracts")
            .join(name)
    }

    fn assert_invalid_numeric_constraint(fixture: &str, expected_column: &str) {
        let error = load_contract(&contract_fixture(fixture))
            .expect_err("invalid numeric constraint should be rejected");

        assert!(
            matches!(
                &error,
                ContractError::InvalidNumericConstraint { column, .. }
                    if column == expected_column
            ),
            "expected InvalidNumericConstraint for '{expected_column}', got: {error}"
        );
    }

    #[test]
    fn load_contract_parses_valid_toml() {
        let contract = load_contract(&contract_fixture("customers.toml"))
            .expect("valid contract should parse");
        assert_eq!(contract.version, 1);
        assert_eq!(contract.name, "customers");
        assert_eq!(
            contract.schema.required_columns,
            vec!["customer_id".to_owned(), "email".to_owned()]
        );
        assert_eq!(contract.completeness, None);
        assert_eq!(contract.types, None);
        assert_eq!(contract.numeric_constraints, None);
    }

    #[test]
    fn load_contract_returns_read_error_for_missing_file() {
        let path = contract_fixture("missing.toml");
        assert!(!path.exists(), "missing fixture must not exist");

        let error = load_contract(&path).expect_err("missing file should fail");

        assert!(matches!(&error, ContractError::ReadFile { .. }));
        assert!(error.to_string().contains("missing.toml"));
    }

    #[test]
    fn load_contract_returns_parse_error_for_malformed_toml() {
        let error = load_contract(&contract_fixture("malformed.toml"))
            .expect_err("malformed TOML should fail");

        assert!(matches!(&error, ContractError::ParseToml { .. }));
        assert!(error.to_string().contains("malformed.toml"));
    }

    #[test]
    fn load_contract_rejects_unsupported_versions() {
        let error = load_contract(&contract_fixture("unsupported_version.toml"))
            .expect_err("unsupported version should fail");

        assert!(matches!(
            &error,
            ContractError::UnsupportedVersion { version: 2, .. }
        ));

        assert!(error.to_string().contains("supported version is 1"));
    }

    #[test]
    fn load_contract_rejects_duplicate_required_columns() {
        let path = contract_fixture("duplicate_required_columns.toml");

        let error = load_contract(&path).expect_err("duplicate required columns should fail");

        assert!(
            matches!(&error, ContractError::DuplicateRequiredColumn {column, ..} if column == "email")
        );
        assert!(
            error
                .to_string()
                .contains("duplicate_required_columns.toml")
        )
    }

    #[test]
    fn load_contract_parses_completeness_constraints() {
        let contract = load_contract(&contract_fixture("customers_completeness.toml"))
            .expect("completeness contract should parse");

        let completeness = contract
            .completeness
            .expect("completeness section should be present");

        assert_eq!(
            completeness.non_empty_columns,
            vec!["customer_id".to_owned(), "email".to_owned()]
        );
    }

    #[test]
    fn load_contract_rejects_completeness_column_not_required() {
        let path = contract_fixture("completeness_column_not_required.toml");

        let error =
            load_contract(&path).expect_err("completeness columns must be required schema columns");

        assert!(
            error.to_string().contains("email"),
            "error should identify the invalid column: {error}"
        );
    }

    #[test]
    fn load_contract_rejects_duplicate_non_empty_columns() {
        let path = contract_fixture("duplicate_non_empty_columns.toml");

        let error = load_contract(&path).expect_err("duplicate completeness columns should fail");

        assert!(
            error.to_string().contains("email"),
            "error should identify the duplicated column: {error}"
        );
    }

    #[test]
    fn load_contract_parses_declared_column_types() {
        let contract = load_contract(&contract_fixture("typed_customers.toml"))
            .expect("valid typed contract should parse");

        let types = contract.types.expect("types section should be present");

        assert_eq!(types.columns.get("customer_id"), Some(&ColumnType::Integer));
        assert_eq!(types.columns.get("age"), Some(&ColumnType::Number));
        assert_eq!(types.columns.get("active"), Some(&ColumnType::Boolean));
    }

    #[test]
    fn load_contract_rejects_unsupported_column_types() {
        let error = load_contract(&contract_fixture("unsupported_column_type.toml"))
            .expect_err("unsupported column type should fail");

        assert!(
            matches!(&error, ContractError::ParseToml { .. }),
            "expected a TOML parsing error, got: {error}"
        );
    }

    #[test]
    fn load_contract_rejects_typed_column_that_is_not_required() {
        let error = load_contract(&contract_fixture("typed_column_not_required.toml"))
            .expect_err("typed column must also be required");

        assert!(
            matches!(
                &error,
                ContractError::TypeColumnNotRequired { column, .. }
                    if column == "age"
            ),
            "expected TypeColumnNotRequired for 'age', got: {error}"
        );
    }

    #[test]
    fn load_contract_parses_numeric_constraints() {
        let contract = load_contract(&contract_fixture("numeric_constraints.toml"))
            .expect("valid numeric constraints should parse");

        let constraints = contract
            .numeric_constraints
            .expect("numeric constraints section should be present");

        let age = constraints
            .columns
            .get("age")
            .expect("age constraint should exist");

        assert_eq!(age.min, Some(NumericLiteral::Integer(18)));
        assert_eq!(age.max, Some(NumericLiteral::Integer(120)));

        let score = constraints
            .columns
            .get("score")
            .expect("score constraint should exist");

        assert_eq!(score.min_exclusive, Some(NumericLiteral::Float(0.0)));
        assert_eq!(score.max, Some(NumericLiteral::Float(100.0)));
    }

    #[test]
    fn load_contract_rejects_unknown_numeric_constraint_fields() {
        let error = load_contract(&contract_fixture("numeric_constraint_unknown_field.toml"))
            .expect_err("unknown numeric constraint fields should fail");

        assert!(
            matches!(&error, ContractError::ParseToml { .. }),
            "expected ParseToml error, got: {error}"
        );
    }

    #[test]
    fn load_contract_rejects_empty_numeric_constraints() {
        let error = load_contract(&contract_fixture("numeric_constraint_empty.toml"))
            .expect_err("empty numeric constraints should fail");

        assert!(
            matches!(
                &error,
                ContractError::InvalidNumericConstraint { column, .. }
                    if column == "age"
            ),
            "expected InvalidNumericConstraint for 'age', got: {error}"
        );
    }

    #[test]
    fn load_contract_rejects_conflicting_lower_bounds() {
        let error = load_contract(&contract_fixture(
            "numeric_constraint_conflicting_lower_bounds.toml",
        ))
        .expect_err("conflicting lower bounds should fail");

        assert!(
            matches!(
                &error,
                ContractError::InvalidNumericConstraint { column, .. }
                    if column == "age"
            ),
            "expected InvalidNumericConstraint for 'age', got: {error}"
        );
    }

    #[test]
    fn load_contract_rejects_conflicting_upper_bounds() {
        let error = load_contract(&contract_fixture(
            "numeric_constraint_conflicting_upper_bounds.toml",
        ))
        .expect_err("conflicting upper bounds should fail");

        assert!(
            matches!(
                &error,
                ContractError::InvalidNumericConstraint { column, .. }
                    if column == "age"
            ),
            "expected InvalidNumericConstraint for 'age', got: {error}"
        );
    }

    #[test]
    fn load_contract_rejects_numeric_constraints_on_boolean_columns() {
        let error = load_contract(&contract_fixture("numeric_constraint_boolean_column.toml"))
            .expect_err("numeric constraints on boolean columns should fail");

        assert!(
            matches!(
                &error,
                ContractError::InvalidNumericConstraint { column, .. }
                    if column == "active"
            ),
            "expected InvalidNumericConstraint for 'active', got: {error}"
        );
    }

    #[test]
    fn load_contract_rejects_numeric_constraints_without_declared_types() {
        assert_invalid_numeric_constraint("numeric_constraint_untyped_column.toml", "age");
    }

    #[test]
    fn load_contract_rejects_float_bounds_for_integer_columns() {
        assert_invalid_numeric_constraint("numeric_constraint_float_bound_for_integer.toml", "age");
    }

    #[test]
    fn load_contract_rejects_nan_bounds() {
        assert_invalid_numeric_constraint("numeric_constraint_nan_bound.toml", "score");
    }

    #[test]
    fn load_contract_rejects_infinite_bounds() {
        assert_invalid_numeric_constraint("numeric_constraint_infinite_bound.toml", "score");
    }

    #[test]
    fn load_contract_rejects_reversed_bounds() {
        assert_invalid_numeric_constraint("numeric_constraint_reversed_bounds.toml", "age");
    }

    #[test]
    fn load_contract_rejects_exclusive_integer_singleton_ranges() {
        assert_invalid_numeric_constraint(
            "numeric_constraint_exclusive_integer_singleton.toml",
            "age",
        );
    }

    #[test]
    fn load_contract_rejects_exclusive_i64_max() {
        assert_invalid_numeric_constraint("numeric_constraint_exclusive_i64_max.toml", "age");
    }

    #[test]
    fn load_contract_rejects_empty_f64_intervals() {
        assert_invalid_numeric_constraint("numeric_constraint_empty_float_interval.toml", "score");
    }

    #[test]
    fn load_contract_accepts_equal_inclusive_integer_bounds() {
        let contract = load_contract(&contract_fixture(
            "numeric_constraint_equal_inclusive_bounds.toml",
        ))
        .expect("equal inclusive bounds should be valid");

        let constraints = contract.numeric_constraints.unwrap();
        let age = constraints.columns.get("age").unwrap();

        assert_eq!(age.min, Some(NumericLiteral::Integer(18)));
        assert_eq!(age.max, Some(NumericLiteral::Integer(18)));
    }

    #[test]
    fn load_contract_preserves_integer_bounds_for_number_columns() {
        let contract = load_contract(&contract_fixture(
            "numeric_constraint_integer_bounds_for_number.toml",
        ))
        .expect("integer bounds should be valid for number columns");

        let constraints = contract.numeric_constraints.unwrap();
        let amount = constraints.columns.get("amount").unwrap();

        assert_eq!(
            amount.min,
            Some(NumericLiteral::Integer(9_007_199_254_740_993))
        );

        assert_eq!(
            amount.max,
            Some(NumericLiteral::Integer(9_007_199_254_740_995))
        );
    }

    #[test]
    fn load_contract_rejects_numeric_constraint_column_not_required() {
        let error = load_contract(&contract_fixture(
            "numeric_constraint_column_not_required.toml",
        ))
        .expect_err("numeric constraint columns must be required");

        assert!(
            matches!(
                &error,
                ContractError::NumericConstraintColumnNotRequired { column, .. }
                    if column == "age"
            ),
            "expected NumericConstraintColumnNotRequired for 'age', got: {error:?}"
        );
    }
}
