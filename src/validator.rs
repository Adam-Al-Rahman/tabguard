use std::{error::Error, fmt, path::Path};

use crate::{
    contract::{self, ContractError},
    inspector::{self, InspectionError},
};

#[derive(Debug)]
pub(crate) struct ValidationReport {
    pub(crate) contract_name: String,
    pub(crate) required_columns: usize,
    pub(crate) present_required_columns: usize,
    pub(crate) data_records: u64,
    pub(crate) missing_columns: Vec<String>,
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
    let inspection = inspector::inspect_csv(csv_path)?;
    let required_columns = contract.schema.required_columns;

    let mut missing_columns = Vec::new();

    for required_column in &required_columns {
        if !inspection.headers.contains(required_column) {
            missing_columns.push(required_column.clone());
        }
    }

    let present_required_columns = required_columns.len() - missing_columns.len();

    Ok(ValidationReport {
        contract_name: contract.name,
        required_columns: required_columns.len(),
        present_required_columns,
        data_records: inspection.data_records,
        missing_columns,
    })
}
