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
                    let mut is_first = true;
                    for value in fields {
                        if !is_first {
                            writer.write_all(app_options.output_greedy_delimiter.as_bytes())?;
                        }
                        writer.write_all(value.as_bytes())?;
                        is_first = false;
                    }
                }
            }
            FieldMap::Greedy { name } => {
                if is_json {
                    serialize_json_value(writer, name)?;
                    writer.write_all(b":")?;
                    serialize_json_array(writer, fields)?;
                } else {
                    let mut is_first = true;
                    loop {
                        if !is_first {
                            writer.write_all(app_options.output_greedy_delimiter.as_bytes())?;
                        }
                        match fields.next() {
                            Some(value) => writer.write_all(value.as_bytes())?,
                            None => break,
                        }
                        is_first = false;
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
    let expected_columns_count = calculate_expected_columns_count(&settings.mapping)?;

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
            process_record(
                record,
                writer,
                settings,
                &selected_mappings,
                &expected_columns_count
            ),
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
    expected_columns_count: &ExpectedColumnsCount,
) -> AnyResult<()> {
    let fields = record.into_iter();

    if !settings.loose {
        check_columns_count(expected_columns_count, record.len())?;
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

            mapping.write_field(writer, settings, fields_iterator.by_ref())?;
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
            FieldMap::Some { colspan, .. } => {
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
                    FieldMap::Some {
                        name: "field2".to_string(),
                        colspan: 2,
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
                    FieldMap::Some {
                        name: "field2".to_string(),
                        colspan: 2,
                    },
                    FieldMap::Greedy {
                        name: "field3".to_string(),
                    },
                ];
                let expected = calculate_expected_columns_count(&mappings)?;
                assert_eq!(expected, ExpectedColumnsCount::AtLeast(3));
                Ok(())
            }

            #[test]
            fn overflow() {
                let mappings = vec![
                    FieldMap::Some {
                        name: "field1".to_string(),
                        colspan: USIZE_MAX_HALF + 1,
                    },
                    FieldMap::Some {
                        name: "field2".to_string(),
                        colspan: USIZE_MAX_HALF + 1,
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
                    1 => FieldMap::Some { name, colspan },
                    _ => FieldMap::Greedy { name },
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
