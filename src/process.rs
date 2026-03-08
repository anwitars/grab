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
    let mappings: Vec<_> = settings
        .mapping
        .iter()
        .map(|mapping| {
            let is_selected = if mapping.is_placeholder() {
                false
            } else {
                settings
                    .select
                    .as_ref()
                    .map(|s| s.contains(mapping.name()))
                    .unwrap_or(true)
            };

            (mapping, is_selected)
        })
        .collect();

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
            process_record(record, writer, settings, &mappings),
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

/// Calculates the total number of columns that the mapping will consume, if it is not greedy.
fn check_columns_count(mappings: &[FieldMap], input_count: usize) -> AnyResult<()> {
    let mut is_greedy = false;
    let mut count = 0;
    for mapping in mappings {
        match mapping {
            FieldMap::One { .. } => count += 1,
            FieldMap::Some { colspan, .. } => count += *colspan,
            FieldMap::Greedy { .. } => is_greedy = true,
        }
    }

    match (is_greedy, count) {
        (false, expected) if expected == input_count => Ok(()),
        (false, expected) => Err(format!(
            "Expected {} fields based on the mapping, but got {}",
            expected, input_count
        )
        .into()),
        (true, expected) if expected <= input_count => Ok(()),
        (true, expected) => Err(format!(
            "Expected at least {} fields based on the mapping, but got {}",
            expected, input_count
        )
        .into()),
    }
}

fn serialize_json_array<'a>(
    writer: &mut impl Write,
    values: &mut impl Iterator<Item = &'a str>,
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

fn serialize_json_value(writer: &mut impl Write, value: &str) -> AnyResult<()> {
    writer.write_all(b"\"")?;
    writer.write_all(value.as_bytes())?;
    writer.write_all(b"\"")?;

    Ok(())
}

fn serialize_json_field(writer: &mut impl Write, name: &str, value: &str) -> AnyResult<()> {
    serialize_json_value(writer, name)?;
    writer.write_all(b":")?;
    serialize_json_value(writer, value)?;

    Ok(())
}
