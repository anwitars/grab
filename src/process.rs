use crate::{
    error::report_error,
    fields::tokenizer::FieldTokenizer,
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

impl StreamSource {
    pub fn reader(self) -> Box<dyn Read> {
        match self {
            StreamSource::Stdin(stdin) => Box::new(stdin),
            StreamSource::File(file) => Box::new(file),
        }
    }
}
/// A trait that defines how to write fields to the output based on the mapping configuration.
trait FieldWriter<'a> {
    /// Writes the specified fields to the output according to the provided settings.
    /// Only called when the field is selected for output.
    fn write_field(
        &self,
        // The writer to which the output should be written.
        writer: &mut impl Write,
        // The application options that may affect how the field is written (e.g., JSON mode, delimiters).
        app_options: &AppOptions,
        // An iterator over the input fields for the current record.
        fields: &mut impl Iterator<Item = &'a str>,
        // A reusable buffer that can be used to basically anything (but mostly for joining fields)
        buffer: &mut Vec<u8>,
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
        buffer: &mut Vec<u8>,
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
            FieldMap::Array {
                name,
                colspan,
                join,
            } => {
                let mut fields = (0..*colspan).map(|_| fields.next().unwrap_or_default());
                if is_json {
                    serialize_json_value(writer, name)?;
                    writer.write_all(b":")?;
                }
                serialize_array_like_field_map(writer, app_options, &mut fields, buffer, *join)?;
            }
            FieldMap::Greedy { name, join } => {
                if is_json {
                    serialize_json_value(writer, name)?;
                    writer.write_all(b":")?;
                }
                serialize_array_like_field_map(writer, app_options, fields, buffer, *join)?;
            }
        }

        Ok(())
    }

    fn consume_fields(&self, fields: &mut impl Iterator<Item = &'a str>) {
        match self {
            FieldMap::One { .. } => {
                let _ = fields.next();
            }
            FieldMap::Array { colspan, .. } => {
                for _ in 0..*colspan {
                    let _ = fields.next();
                }
            }
            FieldMap::Greedy { .. } => for _ in fields {},
        }
    }
}

/// Helper function to share the same serialization logic for both Array and Greedy field mappings.
fn serialize_array_like_field_map<'a>(
    writer: &mut impl Write,
    app_options: &AppOptions,
    fields: &mut impl Iterator<Item = &'a str>,
    buffer: &mut Vec<u8>,
    join: bool,
) -> AnyResult<()> {
    let is_json = app_options.json;

    if is_json {
        if join {
            buffer.clear();
            let mut is_first = true;
            for value in fields {
                if !is_first {
                    buffer.push(b' ');
                }
                buffer.extend_from_slice(value.as_bytes());
                is_first = false;
            }
            let joined = std::str::from_utf8(buffer)?;

            serialize_json_value(writer, joined)?;
        } else {
            serialize_json_array(writer, fields)?;
        }
    } else {
        let mut is_first = true;
        for value in fields {
            std::str::from_utf8(buffer)?;
            if !is_first {
                let delimiter = app_options.output_greedy_delimiter.as_bytes()[0];
                writer.write_all(&[delimiter])?;
            }
            writer.write_all(value.as_bytes())?;
            is_first = false;
        }
    }

    Ok(())
}

/// Processes the input line by line according to the provided settings and outputs the results.
pub fn process<W: Write>(
    tokenizer: &mut dyn FieldTokenizer,
    writer: &mut W,
    settings: &AppOptions,
) -> AnyResult<()> {
    let selected_mappings = settings.selected_mappings();
    let expected_columns_count = calculate_expected_columns_count(&settings.mapping)?;

    // Buffer to be reused for joining fields in Array and Greedy mappings, to avoid allocating a new buffer for each record.
    let mut buffer = Vec::with_capacity(1024);

    let mut line_number = 1;
    while tokenizer
        .next_record()
        .map_err(|e| report_error(e.to_string(), line_number))
        .unwrap_or(false)
    {
        if line_number < settings.skip.unwrap_or(0) + 1 {
            line_number += 1;
            continue;
        } else if let Some(take) = settings.take {
            if line_number > settings.skip.unwrap_or(0) + take {
                break;
            }
        }

        let fields = tokenizer.fields();
        try_report!(
            process_record(
                fields,
                writer,
                settings,
                &selected_mappings,
                &mut buffer,
                &expected_columns_count
            ),
            line_number
        );
        line_number += 1;
    }

    // let mut reader = csv::ReaderBuilder::new()
    //     .has_headers(false)
    //     .buffer_capacity(64 * 1024)
    //     .delimiter(
    //         *settings
    //             .delimiter
    //             .as_bytes()
    //             .first()
    //             .ok_or("Delimiter cannot be empty")?,
    //     )
    //     .from_reader(reader);

    // for (line_number, record) in reader
    //     .records()
    //     // skip the specified number of lines if the skip option is set
    //     .skip(settings.skip.unwrap_or(0))
    //     // take only the specified number of lines if the take option is set
    //     .take(settings.take.unwrap_or(usize::MAX))
    //     .enumerate()
    // {
    //     let line_number = line_number + 1; // Line numbers are 1-based for user-friendly error reporting
    //     let line_number = line_number + settings.skip.unwrap_or(0); // Adjust line number based on skipped lines
    //     let record = try_report!(record, line_number);
    //     try_report!(
    //         process_record(
    //             record,
    //             writer,
    //             settings,
    //             &selected_mappings,
    //             &expected_columns_count
    //         ),
    //         line_number
    //     );
    // }

    Ok(())
}

fn process_record(
    fields: &[&str],
    writer: &mut impl Write,
    settings: &AppOptions,
    mappings: &[(&FieldMap, bool)],
    buffer: &mut Vec<u8>,
    expected_columns_count: &ExpectedColumnsCount,
) -> AnyResult<()> {
    if !settings.loose {
        check_columns_count(expected_columns_count, fields.len())?;
    }

    if settings.json {
        writer.write_all(b"{")?;
    }

    // let's use an iterator to process the fields according to the mapping, and consume columns as we go
    let mut fields_iterator = fields.iter().copied();
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

            mapping.write_field(writer, settings, fields_iterator.by_ref(), buffer)?;
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
enum CalculateExpectedColumnsCountError {
    #[error("Colspan value is too large and exceeds the maximum allowed usize value")]
    ColspanBiggerThanUsizeMax,
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
enum ExpectedColumnsCount {
    Exact(usize),
    AtLeast(usize),
}

#[derive(Debug, thiserror::Error)]
#[cfg_attr(test, derive(PartialEq))]
enum CheckColumnsCountError {
    #[error("Expected {expected} fields based on the mapping, but got {actual}")]
    NotExact { expected: usize, actual: usize },
    #[error("Expected at least {min} fields based on the mapping, but got {actual}")]
    NotAtLeast { min: usize, actual: usize },
}

fn calculate_expected_columns_count(
    mappings: &[FieldMap],
) -> Result<ExpectedColumnsCount, CalculateExpectedColumnsCountError> {
    let mut is_greedy = false;
    let mut count: usize = 0;
    for mapping in mappings {
        match mapping {
            FieldMap::One { .. } => {
                count = count
                    .checked_add(1)
                    .ok_or(CalculateExpectedColumnsCountError::ColspanBiggerThanUsizeMax)?
            }
            FieldMap::Array { colspan, .. } => {
                count = count
                    .checked_add(*colspan)
                    .ok_or(CalculateExpectedColumnsCountError::ColspanBiggerThanUsizeMax)?
            }
            FieldMap::Greedy { .. } => is_greedy = true,
        }
    }

    if is_greedy {
        Ok(ExpectedColumnsCount::AtLeast(count))
    } else {
        Ok(ExpectedColumnsCount::Exact(count))
    }
}

fn check_columns_count(
    expected: &ExpectedColumnsCount,
    input: usize,
) -> Result<(), CheckColumnsCountError> {
    match *expected {
        ExpectedColumnsCount::Exact(expected) if expected == input => Ok(()),
        ExpectedColumnsCount::Exact(expected) => Err(CheckColumnsCountError::NotExact {
            expected,
            actual: input,
        }),
        ExpectedColumnsCount::AtLeast(min) if min <= input => Ok(()),
        ExpectedColumnsCount::AtLeast(min) => {
            Err(CheckColumnsCountError::NotAtLeast { min, actual: input })
        }
    }
}

fn serialize_json_array(
    writer: &mut impl Write,
    values: &mut impl Iterator<Item = impl AsRef<str>>,
) -> AnyResult<()> {
    // do not use serde_json here, as that would expect a Vec<_> in order to serialize an array,
    // but for performance reasons we want to serialize the array directly from the iterator without collecting it into
    // a Vec<_>
    // and since serializing the array is just writing the values with commas in between,
    // we can do that manually without any overhead
    writer.write_all(b"[")?;
    let mut is_first = true;
    for value in values {
        if !is_first {
            writer.write_all(b",")?;
        }
        is_first = false;
        serialize_json_value(writer, value)?;
    }
    writer.write_all(b"]")?;

    Ok(())
}

fn serialize_json_value(writer: &mut impl Write, value: impl AsRef<str>) -> AnyResult<()> {
    // even though serde_json is used, it almost not add any overhead, and adds safety by proper escaping
    serde_json::to_writer(writer, &value.as_ref()).map_err(|e| e.into())
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

    mod columns {
        use super::*;

        mod count {
            use super::*;
            const USIZE_MAX_HALF: usize = usize::MAX / 2;

            #[test]
            fn exact() -> AnyResult<()> {
                let mappings = vec![
                    FieldMap::One {
                        name: "field1".to_string(),
                    },
                    FieldMap::Array {
                        name: "field2".to_string(),
                        colspan: 2,
                        join: false,
                    },
                ];
                let expected = calculate_expected_columns_count(&mappings)?;
                assert_eq!(expected, ExpectedColumnsCount::Exact(3));
                Ok(())
            }

            #[test]
            fn greedy() -> AnyResult<()> {
                let mappings = vec![
                    FieldMap::One {
                        name: "field1".to_string(),
                    },
                    FieldMap::Array {
                        name: "field2".to_string(),
                        colspan: 2,
                        join: false,
                    },
                    FieldMap::Greedy {
                        name: "field3".to_string(),
                        join: false,
                    },
                ];
                let expected = calculate_expected_columns_count(&mappings)?;
                assert_eq!(expected, ExpectedColumnsCount::AtLeast(3));
                Ok(())
            }

            #[test]
            fn overflow() {
                let mappings = vec![
                    FieldMap::Array {
                        name: "field1".to_string(),
                        colspan: USIZE_MAX_HALF + 1,
                        join: false,
                    },
                    FieldMap::Array {
                        name: "field2".to_string(),
                        colspan: USIZE_MAX_HALF + 1,
                        join: false,
                    },
                ];
                let result = calculate_expected_columns_count(&mappings);
                assert_eq!(
                    result,
                    Err(CalculateExpectedColumnsCountError::ColspanBiggerThanUsizeMax)
                );
            }
        }

        mod check {
            use super::*;

            #[test]
            fn exact_match() {
                let expected = ExpectedColumnsCount::Exact(3);
                assert!(check_columns_count(&expected, 3).is_ok());
            }

            #[test]
            fn exact_mismatch() {
                let expected = ExpectedColumnsCount::Exact(3);
                let result = check_columns_count(&expected, 4);
                assert_eq!(
                    result,
                    Err(CheckColumnsCountError::NotExact {
                        expected: 3,
                        actual: 4
                    })
                );
            }

            #[test]
            fn at_least_match() {
                let expected = ExpectedColumnsCount::AtLeast(3);
                assert!(check_columns_count(&expected, 3).is_ok());
                assert!(check_columns_count(&expected, 4).is_ok());
            }

            #[test]
            fn at_least_mismatch() {
                let expected = ExpectedColumnsCount::AtLeast(3);
                let result = check_columns_count(&expected, 2);
                assert_eq!(
                    result,
                    Err(CheckColumnsCountError::NotAtLeast { min: 3, actual: 2 })
                );
            }
        }
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    mod calculate_columns {
        use super::*;

        prop_compose! {
            fn arbitrary_fieldmap()(
                kind in 0u8..3u8,
                colspan in 1..usize::MAX,
                name in ".*"
            ) -> FieldMap {
                match kind {
                    0 => FieldMap::One { name },
                    1 => FieldMap::Array { name, colspan, join: false },
                    _ => FieldMap::Greedy { name, join: false },
                }
            }
        }

        proptest! {
            #[test]
            fn never_panics(mappings in prop::collection::vec(arbitrary_fieldmap(), 1..100)) {
                let result = calculate_expected_columns_count(&mappings);

                // We expect this function to never panic, even if it returns an error for overflow cases
                prop_assert!(result.is_ok() || matches!(result, Err(CalculateExpectedColumnsCountError::ColspanBiggerThanUsizeMax)));
            }
        }
    }

    mod json {
        use super::*;

        proptest! {
            #[test]
            fn serialize_value(value in ".*") {
                let mut output = Vec::new();

                let result = serialize_json_value(&mut output, &value);
                prop_assert!(result.is_ok());

                let expected: serde_json::Value = serde_json::from_slice(&output).unwrap();
                prop_assert_eq!(expected, serde_json::Value::String(value));
            }

            #[test]
            fn serialize_array(values in prop::collection::vec(".*", 0..100)) {
                let mut output = Vec::new();
                let result = serialize_json_array(&mut output, &mut values.iter());
                prop_assert!(result.is_ok());

                let expected: serde_json::Value = serde_json::from_slice(&output).unwrap();
                prop_assert_eq!(
                    expected,
                    serde_json::Value::Array(
                        values
                            .iter()
                            .map(|v| serde_json::Value::String(v.clone()))
                            .collect()
                    )
                );
            }

            #[test]
            fn serialize_field(name in ".*", value in ".*") {
                let mut output = Vec::new();
                let result = serialize_json_field(&mut output, &name, &value);
                prop_assert!(result.is_ok());

                let json_str = format!("{{{}}}", String::from_utf8(output).unwrap());
                let parsed: serde_json::Value = serde_json::from_str(json_str.as_str()).unwrap();

                let expected = serde_json::json!({ name: value });
                prop_assert_eq!(parsed, expected);
            }
        }
    }
}
