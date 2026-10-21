use std::{error::Error, fmt, path::Path};

use crate::{
    contract::{self, ContractError},
    inspector::{self, InspectionError},
};

#[derive(Debug)]
pub(crate) struct NonEmptyColumnViolation {
    pub(crate) column: String,
    pub(crate) empty_values: u64,
}

#[derive(Debug)]
pub(crate) struct ValidationReport {
    pub(crate) contract_name: String,
    pub(crate) required_columns: usize,
    pub(crate) present_required_columns: usize,
    pub(crate) data_records: u64,
    pub(crate) missing_columns: Vec<String>,
    pub(crate) duplicate_headers: Vec<inspector::DuplicateHeader>,
    pub(crate) non_empty_violations: Vec<NonEmptyColumnViolation>,
}

#[derive(Debug)]
pub(crate) enum ValidationError {
    Contract(ContractError),
    Inspection(InspectionError),
}

impl From<ContractError> for ValidationError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

impl From<InspectionError> for ValidationError {
    fn from(error: InspectionError) -> Self {
        Self::Inspection(error)
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(source) => write!(f, "{source}"),
            Self::Inspection(source) => write!(f, "{source}"),
        }
    }
}

impl Error for ValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(source) => Some(source),
            Self::Inspection(source) => Some(source),
        }
    }
}

pub(crate) fn validate_csv(
    csv_path: &Path,
    contract_path: &Path,
) -> Result<ValidationReport, ValidationError> {
    let contract = contract::load_contract(contract_path)?;

    // Count values only in columns requested by the contract.
    let inspection = {
        let non_empty_columns: &[String] =
            contract.completeness.as_ref().map_or(&[], |completeness| {
                completeness.non_empty_columns.as_slice()
            });

        inspector::inspect_csv_with_non_empty_columns(csv_path, non_empty_columns)?
    };

    let required_columns = contract.schema.required_columns;
    let mut missing_columns = Vec::new();

    for required_column in &required_columns {
        if !inspection.headers.contains(required_column) {
            missing_columns.push(required_column.clone());
        }
    }

    let present_required_columns = required_columns.len() - missing_columns.len();

    let mut non_empty_violations = Vec::new();

    for count in &inspection.non_empty_column_counts {
        let empty_values = inspection.data_records - count.non_empty_records;

        if empty_values > 0 {
            non_empty_violations.push(NonEmptyColumnViolation {
                column: count.name.clone(),
                empty_values,
            });
        }
    }

    Ok(ValidationReport {
        contract_name: contract.name,
        required_columns: required_columns.len(),
        present_required_columns,
        data_records: inspection.data_records,
        missing_columns,
        duplicate_headers: inspection.duplicate_headers,
        non_empty_violations,
    })
}
