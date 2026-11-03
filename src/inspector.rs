use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use crate::contract::{ColumnType, NumericConstraint, NumericLiteral};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuoteState {
    FieldStart,
    UnquotedField,
    QuotedField,
    AfterQuote,
}

struct QuoteValidationReader<R> {
    inner: R,
    state: QuoteState,
    byte_offset: u64,
    failure: Option<String>,
}

impl<R> QuoteValidationReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            state: QuoteState::FieldStart,
            byte_offset: 0,
            failure: None,
        }
    }

    fn fail(&mut self, message: String) -> io::Result<usize> {
        self.failure = Some(message.clone());

        Err(io::Error::new(io::ErrorKind::InvalidData, message))
    }
}

impl<R: Read> Read for QuoteValidationReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }

        if let Some(message) = self.failure.clone() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, message));
        }

        let bytes_read = self.inner.read(buffer)?;

        if bytes_read == 0 {
            if self.state == QuoteState::QuotedField {
                return self.fail(format!(
                    "invalid CSV quoting after byte {}: \
                     quoted field was not closed before end of file",
                    self.byte_offset,
                ));
            }

            return Ok(0);
        }

        for &byte in &buffer[..bytes_read] {
            self.byte_offset = self.byte_offset.saturating_add(1);

            let next_state = match self.state {
                QuoteState::FieldStart => match byte {
                    b'"' => QuoteState::QuotedField,
                    b',' | b'\r' | b'\n' => QuoteState::FieldStart,
                    _ => QuoteState::UnquotedField,
                },

                QuoteState::UnquotedField => match byte {
                    b'"' => {
                        return self.fail(format!(
                            "invalid CSV quoting at byte {}: \
                             quote inside an unquoted field",
                            self.byte_offset,
                        ));
                    }
                    b',' | b'\r' | b'\n' => QuoteState::FieldStart,
                    _ => QuoteState::UnquotedField,
                },

                QuoteState::QuotedField => {
                    if byte == b'"' {
                        QuoteState::AfterQuote
                    } else {
                        QuoteState::QuotedField
                    }
                }

                QuoteState::AfterQuote => match byte {
                    b'"' => QuoteState::QuotedField,
                    b',' | b'\r' | b'\n' => QuoteState::FieldStart,
                    _ => {
                        return self.fail(format!(
                            "invalid CSV quoting at byte {}: \
                             unexpected character after a closing quote",
                            self.byte_offset,
                        ));
                    }
                },
            };

            self.state = next_state;
        }

        Ok(bytes_read)
    }
}

#[derive(Debug)]
pub(crate) struct DuplicateHeader {
    pub(crate) name: String,
    pub(crate) first_column: usize,
    pub(crate) duplicate_column: usize,
}

#[derive(Debug)]
pub(crate) struct NonEmptyColumnCount {
    pub(crate) name: String,
    pub(crate) non_empty_records: u64,
}

#[derive(Debug)]
struct SelectedTypedColumn {
    name: String,
    column_type: ColumnType,
    column_index: Option<usize>,
    invalid_values: u64,
}

#[derive(Debug)]
struct SelectedNumericColumn {
    name: String,
    column_type: ColumnType,
    constraint: NumericConstraint,
    column_index: Option<usize>,
    invalid_values: u64,
}

#[derive(Debug)]
pub(crate) struct TypeColumnCount {
    pub(crate) name: String,
    pub(crate) invalid_values: u64,
}

#[derive(Debug)]
pub(crate) struct NumericConstraintCount {
    pub(crate) name: String,
    pub(crate) invalid_values: u64,
}

#[derive(Debug)]
pub(crate) struct CsvInspection {
    pub(crate) headers: Vec<String>,
    pub(crate) data_records: u64,
    pub(crate) duplicate_headers: Vec<DuplicateHeader>,
    pub(crate) non_empty_column_counts: Vec<NonEmptyColumnCount>,
    pub(crate) type_column_counts: Vec<TypeColumnCount>,
    pub(crate) numeric_constraint_counts: Vec<NumericConstraintCount>,
}

#[derive(Debug)]
pub(crate) struct InspectionError {
    path: PathBuf,
    source: csv::Error,
}

impl InspectionError {
    fn new(path: &Path, source: csv::Error) -> Self {
        Self {
            path: path.to_owned(),
            source,
        }
    }
}

impl fmt::Display for InspectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "could not inspect csv '{}': {}",
            self.path.display(),
            self.source
        )
    }
}

impl Error for InspectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

pub(crate) fn inspect_csv_with_constraints(
    path: &Path,
    non_empty_columns: &[String],
    typed_columns: &BTreeMap<String, ColumnType>,
    numeric_constraints: &BTreeMap<String, NumericConstraint>,
) -> Result<CsvInspection, InspectionError> {
    let file =
        File::open(path).map_err(|source| InspectionError::new(path, csv::Error::from(source)))?;

    let validating_reader = QuoteValidationReader::new(file);
    let mut reader = csv::Reader::from_reader(validating_reader);

    let header_record = reader
        .headers()
        .map_err(|source| InspectionError::new(path, source))?
        .clone();

    let headers: Vec<String> = header_record.iter().map(str::to_owned).collect();
    let duplicate_headers = find_duplicate_headers(&headers);

    // Find each requested column's index. A duplicated header is ambiguous,
    // so do not count values for that column.
    let selected_column_indices: Vec<Option<usize>> = non_empty_columns
        .iter()
        .map(|name| {
            if duplicate_headers
                .iter()
                .any(|duplicate| duplicate.name.as_str() == name.as_str())
            {
                None
            } else {
                headers.iter().position(|header| header == name)
            }
        })
        .collect();

    let mut selected_typed_columns: Vec<SelectedTypedColumn> = typed_columns
        .iter()
        .map(|(name, column_type)| {
            // Duplicate headers make a column name ambiguous.
            let column_index = if duplicate_headers
                .iter()
                .any(|duplicate| duplicate.name.as_str() == name.as_str())
            {
                None
            } else {
                headers.iter().position(|header| header == name)
            };

            SelectedTypedColumn {
                name: name.clone(),
                column_type: *column_type,
                column_index,
                invalid_values: 0,
            }
        })
        .collect();

    let mut selected_numeric_columns: Vec<SelectedNumericColumn> = numeric_constraints
        .iter()
        .map(|(name, constraint)| {
            let column_type = typed_columns
                .get(name)
                .copied()
                .expect("numeric constraints must have a validated column type");

            // Duplicate headers make a column name ambiguous.
            let column_index = if duplicate_headers
                .iter()
                .any(|duplicate| duplicate.name.as_str() == name.as_str())
            {
                None
            } else {
                headers.iter().position(|header| header == name)
            };

            SelectedNumericColumn {
                name: name.clone(),
                column_type,
                constraint: *constraint,
                column_index,
                invalid_values: 0,
            }
        })
        .collect();

    let mut non_empty_records = vec![0_u64; non_empty_columns.len()];
    let mut data_records = 0_u64;

    for record in reader.records() {
        let record = record.map_err(|source| InspectionError::new(path, source))?;
        data_records += 1;

        for (count, column_index) in non_empty_records.iter_mut().zip(&selected_column_indices) {
            let Some(column_index) = column_index else {
                continue;
            };

            if record
                .get(*column_index)
                .is_some_and(|value| !value.trim().is_empty())
            {
                *count += 1;
            }
        }

        for column in &mut selected_typed_columns {
            let Some(column_index) = column.column_index else {
                continue;
            };

            let Some(value) = record.get(column_index) else {
                continue;
            };

            if !value.trim().is_empty() && !is_valid_typed_value(column.column_type, value) {
                column.invalid_values += 1;
            }
        }

        for column in &mut selected_numeric_columns {
            let Some(column_index) = column.column_index else {
                continue;
            };

            let Some(value) = record.get(column_index) else {
                continue;
            };

            let value = value.trim();

            // Blank values are handled by completeness validation.
            if value.is_empty() {
                continue;
            }

            // Do not double-count values that already fail type validation.
            if !is_valid_typed_value(column.column_type, value) {
                continue;
            }

            if !is_within_numeric_constraint(column.column_type, value, column.constraint) {
                column.invalid_values += 1;
            }
        }
    }

    let type_column_counts = selected_typed_columns
        .into_iter()
        .filter(|column| column.column_index.is_some())
        .map(|column| TypeColumnCount {
            name: column.name,
            invalid_values: column.invalid_values,
        })
        .collect();

    let numeric_constraint_counts = selected_numeric_columns
        .into_iter()
        .filter(|column| column.column_index.is_some())
        .map(|column| NumericConstraintCount {
            name: column.name,
            invalid_values: column.invalid_values,
        })
        .collect();

    let non_empty_column_counts = non_empty_columns
        .iter()
        .zip(selected_column_indices)
        .zip(non_empty_records)
        .filter_map(|((name, column_index), non_empty_records)| {
            column_index.map(|_| NonEmptyColumnCount {
                name: name.clone(),
                non_empty_records,
            })
        })
        .collect();

    Ok(CsvInspection {
        headers,
        data_records,
        duplicate_headers,
        non_empty_column_counts,
        type_column_counts,
        numeric_constraint_counts,
    })
}

pub(crate) fn inspect_csv(path: &Path) -> Result<CsvInspection, InspectionError> {
    inspect_csv_with_constraints(path, &[], &BTreeMap::new(), &BTreeMap::new())
}

fn find_duplicate_headers(headers: &[String]) -> Vec<DuplicateHeader> {
    let mut first_columns = HashMap::<&str, usize>::new();
    let mut duplicates = Vec::new();
    for (index, name) in headers.iter().enumerate() {
        let column = index + 1;
        if let Some(first_column) = first_columns.get(name.as_str()).copied() {
            duplicates.push(DuplicateHeader {
                name: name.clone(),
                first_column,
                duplicate_column: column,
            });
        } else {
            first_columns.insert(name.as_str(), column);
        }
    }
    duplicates
}

fn is_valid_typed_value(column_type: ColumnType, value: &str) -> bool {
    let value = value.trim();

    match column_type {
        ColumnType::Integer => value.parse::<i64>().is_ok(),
        ColumnType::Number => {
            matches!(value.parse::<f64>(), Ok(number) if number.is_finite())
        }

        ColumnType::Boolean => {
            value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false")
        }
    }
}

fn compare_f64_to_i64(value: f64, integer: i64) -> Ordering {
    // i64::MIN is exactly representable as f64.
    let min_i64 = i64::MIN as f64;

    // 2^63 is exactly representable as f64 and is greater
    // than every possible i64 value.
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

fn compare_numeric_value(column_type: ColumnType, value: &str, bound: NumericLiteral) -> Ordering {
    match column_type {
        ColumnType::Integer => {
            let value = value
                .parse::<i64>()
                .expect("value must have passed integer type validation");

            let NumericLiteral::Integer(bound) = bound else {
                unreachable!("integer constraints require integer bounds");
            };

            value.cmp(&bound)
        }

        ColumnType::Number => {
            let value = value
                .parse::<f64>()
                .expect("value must have passed number type validation");

            match bound {
                NumericLiteral::Integer(integer) => compare_f64_to_i64(value, integer),
                NumericLiteral::Float(float) => value
                    .partial_cmp(&float)
                    .expect("numeric bounds must be finite"),
            }
        }

        ColumnType::Boolean => {
            unreachable!("numeric constraints cannot target boolean columns")
        }
    }
}

fn is_within_numeric_constraint(
    column_type: ColumnType,
    value: &str,
    constraint: NumericConstraint,
) -> bool {
    if constraint
        .min
        .is_some_and(|min| compare_numeric_value(column_type, value, min) == Ordering::Less)
    {
        return false;
    }

    if constraint
        .min_exclusive
        .is_some_and(|min| compare_numeric_value(column_type, value, min) != Ordering::Greater)
    {
        return false;
    }

    if constraint
        .max
        .is_some_and(|max| compare_numeric_value(column_type, value, max) == Ordering::Greater)
    {
        return false;
    }

    if constraint
        .max_exclusive
        .is_some_and(|max| compare_numeric_value(column_type, value, max) != Ordering::Less)
    {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        io::{self, Cursor, Read},
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{
        CsvInspection, QuoteValidationReader, inspect_csv_with_constraints, is_valid_typed_value,
    };
    use crate::contract::{ColumnType, NumericConstraint, NumericLiteral};

    static NEXT_CSV_ID: AtomicU64 = AtomicU64::new(0);

    struct CsvFixture {
        path: PathBuf,
    }

    impl CsvFixture {
        fn new(contents: &str) -> Self {
            let id = NEXT_CSV_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "tabguard-inspector-{}-{id}.csv",
                std::process::id()
            ));

            fs::write(&path, contents).expect("CSV fixture should be written");

            Self { path }
        }
    }

    impl Drop for CsvFixture {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    fn numeric_constraint(
        min: Option<NumericLiteral>,
        max: Option<NumericLiteral>,
        min_exclusive: Option<NumericLiteral>,
        max_exclusive: Option<NumericLiteral>,
    ) -> NumericConstraint {
        NumericConstraint {
            min,
            max,
            min_exclusive,
            max_exclusive,
        }
    }

    fn inspect_csv_text(
        contents: &str,
        non_empty_columns: &[String],
        typed_columns: &BTreeMap<String, ColumnType>,
        numeric_constraints: &BTreeMap<String, NumericConstraint>,
    ) -> CsvInspection {
        let fixture = CsvFixture::new(contents);

        inspect_csv_with_constraints(
            &fixture.path,
            non_empty_columns,
            typed_columns,
            numeric_constraints,
        )
        .expect("valid CSV fixture should be inspected")
    }

    fn numeric_count(report: &CsvInspection, column: &str) -> u64 {
        report
            .numeric_constraint_counts
            .iter()
            .find(|count| count.name == column)
            .unwrap_or_else(|| panic!("numeric count for '{column}' should exist"))
            .invalid_values
    }

    fn type_count(report: &CsvInspection, column: &str) -> u64 {
        report
            .type_column_counts
            .iter()
            .find(|count| count.name == column)
            .unwrap_or_else(|| panic!("type count for '{column}' should exist"))
            .invalid_values
    }

    // ------------------------------------------------------------
    // Existing quote-validation tests
    // ------------------------------------------------------------

    #[test]
    fn quote_validator_accepts_valid_csv_across_one_byte_reads() {
        let input: &[u8] = b"id,notes\r\n1001,\"hello \"\"world\"\"\nsecond line\"\r\n";

        let mut reader = QuoteValidationReader::new(Cursor::new(input));
        let mut output = Vec::with_capacity(input.len());
        let mut byte = [0_u8; 1];

        loop {
            match reader.read(&mut byte) {
                Ok(0) => break,
                Ok(1) => output.push(byte[0]),
                Ok(count) => {
                    panic!("expected at most one byte, received {count}");
                }
                Err(error) => {
                    panic!("valid CSV was rejected: {error}");
                }
            }
        }

        assert_eq!(output.as_slice(), input);
    }

    #[test]
    fn quote_validator_rejects_unterminated_quote_at_eof() {
        let input: &[u8] = b"id,notes\r\n1001,\"unfinished quoted field";

        let mut reader = QuoteValidationReader::new(Cursor::new(input));
        let mut output = Vec::with_capacity(input.len());
        let mut byte = [0_u8; 1];

        loop {
            match reader.read(&mut byte) {
                Ok(0) => {
                    panic!("expected an error for an unterminated quoted field");
                }
                Ok(1) => output.push(byte[0]),
                Ok(count) => {
                    panic!("expected at most one byte, received {count}");
                }
                Err(error) => {
                    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
                    assert_eq!(output.as_slice(), input);
                    break;
                }
            }
        }
    }

    // ------------------------------------------------------------
    // Existing column-type tests
    // ------------------------------------------------------------

    #[test]
    fn integer_values_must_fit_signed_64_bit_range() {
        assert!(is_valid_typed_value(ColumnType::Integer, "42"));
        assert!(is_valid_typed_value(ColumnType::Integer, " -42 "));
        assert!(!is_valid_typed_value(ColumnType::Integer, "42.0"));
        assert!(!is_valid_typed_value(
            ColumnType::Integer,
            "9223372036854775808"
        ));
    }

    #[test]
    fn number_values_must_be_finite() {
        assert!(is_valid_typed_value(ColumnType::Number, "42"));
        assert!(is_valid_typed_value(ColumnType::Number, "42.5"));
        assert!(is_valid_typed_value(ColumnType::Number, "1e3"));
        assert!(!is_valid_typed_value(ColumnType::Number, "NaN"));
        assert!(!is_valid_typed_value(ColumnType::Number, "Infinity"));
        assert!(!is_valid_typed_value(ColumnType::Number, "not_a_number"));
    }

    #[test]
    fn boolean_values_are_case_insensitive() {
        assert!(is_valid_typed_value(ColumnType::Boolean, "true"));
        assert!(is_valid_typed_value(ColumnType::Boolean, " FALSE "));
        assert!(is_valid_typed_value(ColumnType::Boolean, "TrUe"));
        assert!(!is_valid_typed_value(ColumnType::Boolean, "yes"));
        assert!(!is_valid_typed_value(ColumnType::Boolean, "1"));
    }

    // ------------------------------------------------------------
    // Numeric inclusive and exclusive boundary tests
    // ------------------------------------------------------------

    #[test]
    fn inspection_accepts_values_at_inclusive_integer_boundaries() {
        let typed_columns = BTreeMap::from([("age".to_owned(), ColumnType::Integer)]);

        let constraints = BTreeMap::from([(
            "age".to_owned(),
            numeric_constraint(
                Some(NumericLiteral::Integer(18)),
                Some(NumericLiteral::Integer(65)),
                None,
                None,
            ),
        )]);

        let report = inspect_csv_text("age\n17\n18\n65\n66\n", &[], &typed_columns, &constraints);

        assert_eq!(report.data_records, 4);
        assert_eq!(numeric_count(&report, "age"), 2);
        assert_eq!(type_count(&report, "age"), 0);
    }

    #[test]
    fn inspection_rejects_values_at_exclusive_integer_boundaries() {
        let typed_columns = BTreeMap::from([("age".to_owned(), ColumnType::Integer)]);

        let constraints = BTreeMap::from([(
            "age".to_owned(),
            numeric_constraint(
                None,
                None,
                Some(NumericLiteral::Integer(18)),
                Some(NumericLiteral::Integer(65)),
            ),
        )]);

        let report = inspect_csv_text("age\n18\n19\n64\n65\n", &[], &typed_columns, &constraints);

        assert_eq!(numeric_count(&report, "age"), 2);
        assert_eq!(type_count(&report, "age"), 0);
    }

    #[test]
    fn inspection_enforces_floating_point_boundaries() {
        let typed_columns = BTreeMap::from([("score".to_owned(), ColumnType::Number)]);

        let constraints = BTreeMap::from([(
            "score".to_owned(),
            numeric_constraint(
                None,
                Some(NumericLiteral::Float(2.5)),
                Some(NumericLiteral::Float(0.5)),
                None,
            ),
        )]);

        let report = inspect_csv_text(
            "score\n0.5\n0.5001\n2.5\n2.6\n",
            &[],
            &typed_columns,
            &constraints,
        );

        assert_eq!(numeric_count(&report, "score"), 2);
        assert_eq!(type_count(&report, "score"), 0);
    }

    // ------------------------------------------------------------
    // Type validation and numeric constraint interaction
    // ------------------------------------------------------------

    #[test]
    fn type_invalid_values_are_not_counted_as_numeric_violations() {
        let typed_columns = BTreeMap::from([("age".to_owned(), ColumnType::Integer)]);

        let constraints = BTreeMap::from([(
            "age".to_owned(),
            numeric_constraint(
                Some(NumericLiteral::Integer(18)),
                Some(NumericLiteral::Integer(20)),
                None,
                None,
            ),
        )]);

        let report = inspect_csv_text(
            "age\nnot-a-number\n9223372036854775808\n9\n21\n18\n",
            &[],
            &typed_columns,
            &constraints,
        );

        assert_eq!(type_count(&report, "age"), 2);
        assert_eq!(numeric_count(&report, "age"), 2);
    }

    #[test]
    fn non_finite_number_values_are_not_counted_as_numeric_violations() {
        let typed_columns = BTreeMap::from([("score".to_owned(), ColumnType::Number)]);

        let constraints = BTreeMap::from([(
            "score".to_owned(),
            numeric_constraint(
                Some(NumericLiteral::Integer(0)),
                Some(NumericLiteral::Integer(100)),
                None,
                None,
            ),
        )]);

        let report = inspect_csv_text(
            "score\nNaN\nInfinity\n-Infinity\n50\n",
            &[],
            &typed_columns,
            &constraints,
        );

        assert_eq!(type_count(&report, "score"), 3);
        assert_eq!(numeric_count(&report, "score"), 0);
    }

    #[test]
    fn blank_values_are_excluded_from_type_and_numeric_violations() {
        let typed_columns = BTreeMap::from([("age".to_owned(), ColumnType::Integer)]);

        let constraints = BTreeMap::from([(
            "age".to_owned(),
            numeric_constraint(
                Some(NumericLiteral::Integer(18)),
                Some(NumericLiteral::Integer(65)),
                None,
                None,
            ),
        )]);

        let non_empty_columns = vec!["age".to_owned()];

        let report = inspect_csv_text(
            "age\n\"\"\n   \n18\n",
            &non_empty_columns,
            &typed_columns,
            &constraints,
        );

        assert_eq!(report.data_records, 3);
        assert_eq!(type_count(&report, "age"), 0);
        assert_eq!(numeric_count(&report, "age"), 0);

        assert_eq!(report.non_empty_column_counts.len(), 1);
        assert_eq!(report.non_empty_column_counts[0].non_empty_records, 1);
    }

    // ------------------------------------------------------------
    // Numeric precision and integer representation
    // ------------------------------------------------------------

    #[test]
    fn number_columns_compare_integer_bounds_without_loss_of_precision() {
        let typed_columns = BTreeMap::from([
            ("lower".to_owned(), ColumnType::Number),
            ("upper".to_owned(), ColumnType::Number),
        ]);

        let exact_bound = 9_007_199_254_740_993_i64;

        let constraints = BTreeMap::from([
            (
                "lower".to_owned(),
                numeric_constraint(Some(NumericLiteral::Integer(exact_bound)), None, None, None),
            ),
            (
                "upper".to_owned(),
                numeric_constraint(None, Some(NumericLiteral::Integer(exact_bound)), None, None),
            ),
        ]);

        let report = inspect_csv_text(
            concat!(
                "lower,upper\n",
                "9007199254740992,9007199254740992\n",
                "9007199254740994,9007199254740994\n",
            ),
            &[],
            &typed_columns,
            &constraints,
        );

        assert_eq!(numeric_count(&report, "lower"), 1);
        assert_eq!(numeric_count(&report, "upper"), 1);
        assert_eq!(type_count(&report, "lower"), 0);
        assert_eq!(type_count(&report, "upper"), 0);
    }

    #[test]
    fn integer_columns_handle_i64_min_and_max_exactly() {
        let typed_columns = BTreeMap::from([("age".to_owned(), ColumnType::Integer)]);

        let constraints = BTreeMap::from([(
            "age".to_owned(),
            numeric_constraint(
                Some(NumericLiteral::Integer(i64::MIN)),
                Some(NumericLiteral::Integer(i64::MAX)),
                None,
                None,
            ),
        )]);

        let report = inspect_csv_text(
            "age\n-9223372036854775808\n9223372036854775807\n",
            &[],
            &typed_columns,
            &constraints,
        );

        assert_eq!(report.data_records, 2);
        assert_eq!(type_count(&report, "age"), 0);
        assert_eq!(numeric_count(&report, "age"), 0);
    }

    // ------------------------------------------------------------
    // Aggregation and deterministic output
    // ------------------------------------------------------------

    #[test]
    fn numeric_violation_counts_are_aggregated_per_column() {
        let typed_columns = BTreeMap::from([
            ("age".to_owned(), ColumnType::Integer),
            ("score".to_owned(), ColumnType::Number),
        ]);

        let constraints = BTreeMap::from([
            (
                "age".to_owned(),
                numeric_constraint(
                    Some(NumericLiteral::Integer(18)),
                    Some(NumericLiteral::Integer(65)),
                    None,
                    None,
                ),
            ),
            (
                "score".to_owned(),
                numeric_constraint(
                    Some(NumericLiteral::Float(0.0)),
                    Some(NumericLiteral::Float(100.0)),
                    None,
                    None,
                ),
            ),
        ]);

        let report = inspect_csv_text(
            "age,score\n17,-1\n18,50\n66,101\n",
            &[],
            &typed_columns,
            &constraints,
        );

        assert_eq!(numeric_count(&report, "age"), 2);
        assert_eq!(numeric_count(&report, "score"), 2);
        assert_eq!(type_count(&report, "age"), 0);
        assert_eq!(type_count(&report, "score"), 0);
    }

    #[test]
    fn numeric_constraint_counts_are_ordered_by_column_name() {
        let typed_columns = BTreeMap::from([
            ("a".to_owned(), ColumnType::Integer),
            ("z".to_owned(), ColumnType::Integer),
        ]);

        let constraints = BTreeMap::from([
            (
                "z".to_owned(),
                numeric_constraint(Some(NumericLiteral::Integer(1)), None, None, None),
            ),
            (
                "a".to_owned(),
                numeric_constraint(Some(NumericLiteral::Integer(1)), None, None, None),
            ),
        ]);

        let report = inspect_csv_text("z,a\n0,0\n", &[], &typed_columns, &constraints);

        let names: Vec<&str> = report
            .numeric_constraint_counts
            .iter()
            .map(|count| count.name.as_str())
            .collect();

        assert_eq!(names, vec!["a", "z"]);
    }

    // ------------------------------------------------------------
    // Header and empty-file behavior
    // ------------------------------------------------------------

    #[test]
    fn missing_numeric_headers_are_not_inspected() {
        let typed_columns = BTreeMap::from([("age".to_owned(), ColumnType::Integer)]);

        let constraints = BTreeMap::from([(
            "age".to_owned(),
            numeric_constraint(Some(NumericLiteral::Integer(18)), None, None, None),
        )]);

        let report = inspect_csv_text("customer_id\n1001\n", &[], &typed_columns, &constraints);

        assert_eq!(report.data_records, 1);
        assert!(report.numeric_constraint_counts.is_empty());
        assert!(report.type_column_counts.is_empty());
    }

    #[test]
    fn duplicate_numeric_headers_are_not_inspected() {
        let typed_columns = BTreeMap::from([("age".to_owned(), ColumnType::Integer)]);

        let constraints = BTreeMap::from([(
            "age".to_owned(),
            numeric_constraint(Some(NumericLiteral::Integer(18)), None, None, None),
        )]);

        let report = inspect_csv_text("age,age\n17,19\n", &[], &typed_columns, &constraints);

        assert_eq!(report.duplicate_headers.len(), 1);
        assert!(report.numeric_constraint_counts.is_empty());
        assert!(report.type_column_counts.is_empty());
    }

    #[test]
    fn empty_csv_is_inspected_without_numeric_violations() {
        let typed_columns = BTreeMap::from([("age".to_owned(), ColumnType::Integer)]);

        let constraints = BTreeMap::from([(
            "age".to_owned(),
            numeric_constraint(
                Some(NumericLiteral::Integer(18)),
                Some(NumericLiteral::Integer(65)),
                None,
                None,
            ),
        )]);

        let report = inspect_csv_text("", &[], &typed_columns, &constraints);

        assert_eq!(report.data_records, 0);
        assert!(report.numeric_constraint_counts.is_empty());
    }

    #[test]
    fn headers_only_csv_has_zero_numeric_violations() {
        let typed_columns = BTreeMap::from([("age".to_owned(), ColumnType::Integer)]);

        let constraints = BTreeMap::from([(
            "age".to_owned(),
            numeric_constraint(
                Some(NumericLiteral::Integer(18)),
                Some(NumericLiteral::Integer(65)),
                None,
                None,
            ),
        )]);

        let report = inspect_csv_text("age\n", &[], &typed_columns, &constraints);

        assert_eq!(report.data_records, 0);
        assert_eq!(numeric_count(&report, "age"), 0);
        assert_eq!(type_count(&report, "age"), 0);
    }
}
