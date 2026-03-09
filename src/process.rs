use crate::{
    options::{AppOptions, FieldMap},
    try_report,
    types::AnyResult,
};
use std::io::{BufReader, Read, StdinLock, Write};

/// Represents the source of the input stream, either from standard input or a file.
#[derive(Debug)]
pub enum StreamSource {
    Stdin(StdinLock<'static>),
    File(BufReader<std::fs::File>),
}

/// A trait that defines how to write fields to the output based on the mapping configuration.
trait FieldWriter<'a> {
    /// Writes the specified fields to the output according to the provided settings.
    /// Only called when the field is selected for output.
    fn write_field(
        &self,
        writer: &mut impl Write,
        app_options: &AppOptions,
        fields: &mut impl Iterator<Item = &'a str>,
    ) -> AnyResult<()>;

    /// Consumes the specified number of fields from the input iterator, effectively skipping them.
    /// Only called when the field is not selected for output.
    fn consume_fields(&self, fields: &mut impl Iterator<Item = &'a str>);
}

impl<'a> FieldWriter<'a> for FieldMap {
    fn write_field(
        &self,
        writer: &mut impl Write,
        app_options: &AppOptions,
        fields: &mut impl Iterator<Item = &'a str>,
    ) -> AnyResult<()> {
        let is_json = app_options.json;

        match self {
            FieldMap::One { name } => {
                let value = fields.next().unwrap_or_default();
                if is_json {
                    serialize_json_field(writer, name, value)?;
                } else {
                    writer.write_all(value.as_bytes())?;
                }
            }
            FieldMap::Some { name, colspan } => {
                let mut fields = (0..*colspan).map(|_| fields.next().unwrap_or_default());
                if is_json {
                    serialize_json_value(writer, name)?;
                    writer.write_all(b":")?;
                    serialize_json_array(writer, &mut fields)?;
                } else {
                    for (i, value) in fields.enumerate() {
                        if i > 0 {
                            writer.write_all(app_options.output_greedy_delimiter.as_bytes())?;
                        }
                        writer.write_all(value.as_bytes())?;
                    }
                }
            }
            FieldMap::Greedy { name } => {
                if is_json {
                    serialize_json_value(writer, name)?;
                    writer.write_all(b":")?;
                    serialize_json_array(writer, fields)?;
                } else {
                    for i in 0.. {
                        if i > 0 {
                            writer.write_all(app_options.output_greedy_delimiter.as_bytes())?;
                        }
                        match fields.next() {
                            Some(value) => writer.write_all(value.as_bytes())?,
                            None => break,
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn consume_fields(&self, fields: &mut impl Iterator<Item = &'a str>) {
        match self {
            FieldMap::One { .. } => {
                let _ = fields.next();
            }
            FieldMap::Some { colspan, .. } => {
                for _ in 0..*colspan {
                    let _ = fields.next();
                }
            }
            FieldMap::Greedy { .. } => for _ in fields {},
        }
    }
}

/// Processes the input line by line according to the provided settings and outputs the results.
pub fn process<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    settings: &AppOptions,
) -> AnyResult<()> {
    let selected_mappings = settings.selected_mappings();

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .buffer_capacity(64 * 1024)
        .delimiter(
            *settings
                .delimiter
                .as_bytes()
                .first()
                .ok_or("Delimiter cannot be empty")?,
        )
        .from_reader(reader);

    for (line_number, record) in reader
        .records()
        // skip the specified number of lines if the skip option is set
        .skip(settings.skip.unwrap_or(0))
        // take only the specified number of lines if the take option is set
        .take(settings.take.unwrap_or(usize::MAX))
        .enumerate()
    {
        let line_number = line_number + 1; // Line numbers are 1-based for user-friendly error reporting
        let line_number = line_number + settings.skip.unwrap_or(0); // Adjust line number based on skipped lines
        let record = try_report!(record, line_number);
        try_report!(
            process_record(record, writer, settings, &selected_mappings),
            line_number
        );
    }

    Ok(())
}

fn process_record(
    record: csv::StringRecord,
    writer: &mut impl Write,
    settings: &AppOptions,
    mappings: &[(&FieldMap, bool)],
) -> AnyResult<()> {
    let fields = record.into_iter();

    if !settings.loose {
        check_columns_count(&settings.mapping, record.len())?;
    }

    if settings.json {
        writer.write_all(b"{")?;
    }

    // let's use an iterator to process the fields according to the mapping, and consume columns as we go
    let mut fields_iterator = fields.into_iter();
    let mut is_first = true;

    for (mapping, is_selected) in mappings.iter() {
        if *is_selected {
            if !is_first {
                if settings.json {
                    writer.write_all(b",")?;
                } else {
                    writer.write_all(settings.output_delimiter.as_bytes())?;
                }
            }
            is_first = false;

            mapping.write_field(writer, &settings, fields_iterator.by_ref())?;
        } else {
            mapping.consume_fields(fields_iterator.by_ref());
        }
    }

    if settings.json {
        writer.write_all(b"}")?;
    }
    writer.write_all(b"\n")?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
#[cfg_attr(test, derive(PartialEq))]
enum CheckColumnsCountError {
    #[error("Expected {expected} fields based on the mapping, but got {actual}")]
    NotExact { expected: usize, actual: usize },
    #[error("Expected at least {min} fields based on the mapping, but got {actual}")]
    NotAtLeast { min: usize, actual: usize },
    #[error("Colspan value is too large and exceeds the maximum allowed usize value")]
    ColspanBiggerThanUsizeMax,
}

/// Calculates the total number of columns that the mapping will consume, if it is not greedy.
fn check_columns_count(
    mappings: &[FieldMap],
    input_count: usize,
) -> Result<(), CheckColumnsCountError> {
    let mut is_greedy = false;
    let mut count: usize = 0;
    for mapping in mappings {
        match mapping {
            FieldMap::One { .. } => {
                count = count
                    .checked_add(1)
                    .ok_or(CheckColumnsCountError::ColspanBiggerThanUsizeMax)?
            }
            FieldMap::Some { colspan, .. } => {
                count = count
                    .checked_add(*colspan)
                    .ok_or(CheckColumnsCountError::ColspanBiggerThanUsizeMax)?
            }
            FieldMap::Greedy { .. } => is_greedy = true,
        }
    }

    match (is_greedy, count) {
        (false, expected) if expected == input_count => Ok(()),
        (false, expected) => Err(CheckColumnsCountError::NotExact {
            expected,
            actual: input_count,
        }),
        (true, expected) if expected <= input_count => Ok(()),
        (true, expected) => Err(CheckColumnsCountError::NotAtLeast {
            min: expected,
            actual: input_count,
        }),
    }
}

fn serialize_json_array(
    writer: &mut impl Write,
    values: &mut impl Iterator<Item = impl AsRef<str>>,
) -> AnyResult<()> {
    writer.write_all(b"[")?;
    for (i, value) in values.enumerate() {
        if i > 0 {
            writer.write_all(b",")?;
        }
        serialize_json_value(writer, value)?;
    }
    writer.write_all(b"]")?;

    Ok(())
}

fn serialize_json_value(writer: &mut impl Write, value: impl AsRef<str>) -> AnyResult<()> {
    writer.write_all(b"\"")?;
    writer.write_all(value.as_ref().as_bytes())?;
    writer.write_all(b"\"")?;

    Ok(())
}

fn serialize_json_field(
    writer: &mut impl Write,
    name: impl AsRef<str>,
    value: impl AsRef<str>,
) -> AnyResult<()> {
    serialize_json_value(writer, name)?;
    writer.write_all(b":")?;
    serialize_json_value(writer, value)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod json {
        use super::*;

        #[test]
        fn serialize_value() {
            let mut output = Vec::new();
            serialize_json_value(&mut output, "test").unwrap();
            assert_eq!(String::from_utf8(output).unwrap(), r#""test""#);
        }

        #[test]
        fn serialize_array() {
            let mut output = Vec::new();
            serialize_json_array(&mut output, &mut ["value1", "value2"].into_iter()).unwrap();
            assert_eq!(String::from_utf8(output).unwrap(), r#"["value1","value2"]"#);
        }

        #[test]
        fn serialize_field() {
            let mut output = Vec::new();
            serialize_json_field(&mut output, "name", "value").unwrap();
            assert_eq!(String::from_utf8(output).unwrap(), r#""name":"value""#);
        }
    }

    mod column_count {
        use super::*;

        mod ok {
            use super::*;

            #[test]
            fn exact() {
                let mappings = vec![
                    FieldMap::One {
                        name: "a".to_string(),
                    },
                    FieldMap::Some {
                        name: "b".to_string(),
                        colspan: 2,
                    },
                ];
                assert_eq!(check_columns_count(&mappings, 3), Ok(()));
            }

            #[test]
            fn greedy() {
                let mappings = vec![
                    FieldMap::One {
                        name: "a".to_string(),
                    },
                    FieldMap::Greedy {
                        name: "b".to_string(),
                    },
                ];
                assert_eq!(check_columns_count(&mappings, 5), Ok(()));
            }

            #[test]
            fn greedy_exact() {
                let mappings = vec![
                    FieldMap::One {
                        name: "a".to_string(),
                    },
                    FieldMap::Greedy {
                        name: "b".to_string(),
                    },
                ];
                assert_eq!(check_columns_count(&mappings, 1), Ok(()));
            }
        }

        mod error {
            use super::*;
            const USIZE_MAX_HALF: usize = usize::MAX / 2;

            #[test]
            fn too_few() {
                let mappings = vec![
                    FieldMap::One {
                        name: "a".to_string(),
                    },
                    FieldMap::Some {
                        name: "b".to_string(),
                        colspan: 2,
                    },
                ];
                assert_eq!(
                    check_columns_count(&mappings, 2),
                    Err(CheckColumnsCountError::NotExact {
                        expected: 3,
                        actual: 2,
                    })
                );
            }

            #[test]
            fn too_many() {
                let mappings = vec![
                    FieldMap::One {
                        name: "a".to_string(),
                    },
                    FieldMap::Some {
                        name: "b".to_string(),
                        colspan: 2,
                    },
                ];
                assert_eq!(
                    check_columns_count(&mappings, 4),
                    Err(CheckColumnsCountError::NotExact {
                        expected: 3,
                        actual: 4,
                    })
                );
            }

            #[test]
            fn greedy_too_few() {
                let mappings = vec![
                    FieldMap::One {
                        name: "a".to_string(),
                    },
                    FieldMap::One {
                        name: "b".to_string(),
                    },
                    FieldMap::Greedy {
                        name: "c".to_string(),
                    },
                ];
                assert_eq!(
                    check_columns_count(&mappings, 1),
                    Err(CheckColumnsCountError::NotAtLeast { min: 2, actual: 1 })
                );
            }

            #[test]
            fn colspan_too_large() {
                let mappings = vec![
                    FieldMap::Some {
                        name: "a".to_string(),
                        colspan: USIZE_MAX_HALF,
                    },
                    FieldMap::Some {
                        name: "b".to_string(),
                        colspan: USIZE_MAX_HALF,
                    },
                    FieldMap::Some {
                        name: "c".to_string(),
                        colspan: USIZE_MAX_HALF,
                    },
                ];
                assert_eq!(
                    check_columns_count(&mappings, 0),
                    Err(CheckColumnsCountError::ColspanBiggerThanUsizeMax)
                );
            }
        }
    }
}
