use std::{
    collections::HashSet,
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

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Contract {
    pub(crate) version: u32,
    pub(crate) name: String,
    pub(crate) schema: SchemaContract,

    #[serde(default)]
    pub(crate) completeness: Option<CompletenessContract>,
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
            | Self::CompletenessColumnNotRequired { .. } => None,
        }
    }
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

    let mut seen_columns = HashSet::with_capacity(contract.schema.required_columns.len());

    for column in &contract.schema.required_columns {
        if !seen_columns.insert(column.as_str()) {
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

            if !contract
                .schema
                .required_columns
                .iter()
                .any(|required| required == column)
            {
                return Err(ContractError::CompletenessColumnNotRequired {
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

    use super::{ContractError, load_contract};

    fn contract_fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/contracts")
            .join(name)
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
}
