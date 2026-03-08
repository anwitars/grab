use crate::{
    error::report_error,
    options::{AppOptions, FieldMap},
    try_report,
    types::AnyResult,
};
use std::io::{BufRead, BufReader, BufWriter, Write};

/// Represents the source of the input stream, either from standard input or a file.
#[derive(Debug)]
pub enum StreamSource {
    Stdin(std::io::StdinLock<'static>),
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
                let value = fields.next().ok_or_else(|| {
                    format!("Expected more fields for mapping '{}', but got fewer", name)
                })?;

                if is_json {
                    serialize_json_field(writer, name, value)?;
                } else {
                    writer.write_all(value.as_bytes())?;
                }
            }
            FieldMap::Some { name, colspan } => {
                if is_json {
                    serialize_json_value(writer, name)?;
                    writer.write_all(b":")?;
                    serialize_json_array(writer, &mut fields.take(*colspan))?;
                } else {
                    for (i, value) in fields.take(*colspan).enumerate() {
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
                    for (i, value) in fields.enumerate() {
                        if i > 0 {
                            writer.write_all(app_options.output_greedy_delimiter.as_bytes())?;
                        }
                        writer.write_all(value.as_bytes())?;
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
pub fn process<R: BufRead>(reader: &mut R, settings: &AppOptions) -> AnyResult<()> {
    let mut writer = BufWriter::new(std::io::stdout());

    let mappings: Vec<_> = settings
        .mapping
        .iter()
        .map(|mapping| {
            let is_selected = settings
                .select
                .as_ref()
                .map(|s| s.contains(mapping.name()))
                .unwrap_or(true);

            (mapping, is_selected)
        })
        .collect();

    for (line_number, line) in reader
        .lines()
        // filter out lines that cannot be read
        .filter_map(Result::ok)
        // skip the specified number of lines if the skip option is set
        .skip(settings.skip.unwrap_or(0))
        // take only the specified number of lines if the take option is set
        .take(settings.take.unwrap_or(usize::MAX))
        .enumerate()
    {
        let line_number = line_number + 1; // Line numbers are 1-based for user-friendly error reporting
        let line_number = line_number + settings.skip.unwrap_or(0); // Adjust line number based on skipped lines

        // split by the specified delimiter
        // FIXME: as moved to memchr, it only operates on the first byte of the delimiter
        let delimiter_byte = settings
            .delimiter
            .as_bytes()
            .first()
            .ok_or("Delimiter cannot be empty")?;
        let fields = split_by_delimiter(&line, *delimiter_byte);

        // for now, we always validate the number of fields against the mapping if the mapping is not greedy
        // TODO: implement --strict option to control this behavior
        let mapping_count = mapping_columns_count(&settings.mapping);
        if let Some(count) = mapping_count {
            let mut fields_count = 0;
            for _ in fields.clone() {
                fields_count += 1;
            }
            if fields_count != count {
                report_error(
                    format!(
                        "Expected {} fields based on the mapping, but got {}",
                        count, fields_count
                    ),
                    line_number,
                );
                continue;
            }
        }

        if settings.json {
            try_report!(writer.write_all(b"{"), line_number);
        }

        // let's use an iterator to process the fields according to the mapping, and consume columns as we go
        let mut fields_iterator = fields.into_iter();
        let mut is_first = true;

        for (mapping, is_selected) in mappings.iter() {
            if *is_selected {
                if !is_first {
                    if settings.json {
                        try_report!(writer.write_all(b","), line_number);
                    } else {
                        try_report!(
                            writer.write_all(settings.output_delimiter.as_bytes()),
                            line_number
                        );
                    }
                }
                is_first = false;

                try_report!(
                    mapping.write_field(&mut writer, &settings, fields_iterator.by_ref()),
                    line_number
                );
            } else {
                mapping.consume_fields(fields_iterator.by_ref());
            }
        }

        if settings.json {
            try_report!(writer.write_all(b"}"), line_number);
        }
        try_report!(writer.write_all(b"\n"), line_number);
    }

    Ok(())
}

/// Calculates the total number of columns that the mapping will consume, if it is not greedy.
fn mapping_columns_count(mappings: &[FieldMap]) -> Option<usize> {
    let mut count = 0;
    for mapping in mappings {
        match mapping {
            FieldMap::One { .. } => count += 1,
            FieldMap::Some { colspan, .. } => count += *colspan,
            FieldMap::Greedy { .. } => return None, // Greedy can consume any number of columns
        }
    }
    Some(count)
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

fn split_by_delimiter<'a>(
    line: &'a str,
    delimiter: u8,
) -> impl Iterator<Item = &'a str> + 'a + Clone {
    let mut start = 0;

    std::iter::from_fn(move || {
        if start > line.len() {
            return None;
        }

        let end = match memchr::memchr(delimiter, &line.as_bytes()[start..]) {
            Some(pos) => start + pos,
            None => line.len(),
        };

        let slice = &line[start..end];
        start = end + 1; // Move past the delimiter for the next iteration
        Some(slice)
    })
}
