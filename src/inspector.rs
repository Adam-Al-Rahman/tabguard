use std::{
    collections::HashMap,
    error::Error,
    fmt,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

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
pub(crate) struct CsvInspection {
    pub(crate) headers: Vec<String>,
    pub(crate) data_records: u64,
    pub(crate) duplicate_headers: Vec<DuplicateHeader>,
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

pub(crate) fn inspect_csv(path: &Path) -> Result<CsvInspection, InspectionError> {
    let file =
        File::open(path).map_err(|source| InspectionError::new(path, csv::Error::from(source)))?;

    let validating_reader = QuoteValidationReader::new(file);
    let mut reader = csv::Reader::from_reader(validating_reader);
    let header_record = reader
        .headers()
        .map_err(|source| InspectionError::new(path, source))?
        .clone();

    let headers: Vec<String> = header_record.iter().map(str::to_owned).collect();

    let mut data_records = 0_u64;

    for record in reader.records() {
        record.map_err(|source| InspectionError::new(path, source))?;
        data_records += 1;
    }

    let duplicate_headers = find_duplicate_headers(&headers);
    Ok(CsvInspection {
        headers,
        data_records,
        duplicate_headers,
    })
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

#[cfg(test)]
mod tests {
    use std::io::{self, Cursor, Read};

    use super::QuoteValidationReader;

    #[test]
    fn quote_validator_accepts_valid_csv_across_one_byte_reads() {
        let input: &[u8] = b"id,notes\r\n1001,\"hello \"\"world\"\"\nsecond line\"\r\n";

        let mut reader = QuoteValidationReader::new(Cursor::new(input));
        let mut output = Vec::with_capacity(input.len());
        let mut byte = [0_u8; 1]; // `Read trait` expects a mutable byte slice (&mut [u8])

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
}
