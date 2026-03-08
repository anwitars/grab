use crate::{
    options::{AppOptions, FieldMap},
    types::AnyResult,
};
use std::{
    collections::HashSet,
    io::{BufRead, BufReader, BufWriter, Write},
};

/// Represents the source of the input stream, either from standard input or a file.
#[derive(Debug)]
pub enum StreamSource {
    Stdin(std::io::StdinLock<'static>),
    File(BufReader<std::fs::File>),
}

enum SelectedField {
    One(String),
    Some(Vec<String>),
}

/// Processes the input line by line according to the provided settings and outputs the results.
pub fn process<R: BufRead>(reader: &mut R, settings: &AppOptions) -> AnyResult<()> {
    let mut writer = BufWriter::new(std::io::stdout());

    // determine the set of field names to include in the output based on the select option if the user defined it,
    // otherwise include all fields defined in the mapping.
    let selected_field_names: HashSet<&str> = settings
        .select
        .as_ref()
        .map(|s| s.iter().map(|f| f.as_str()).collect())
        .unwrap_or_else(|| settings.mapping.iter().map(|m| m.name()).collect());

    for line in reader
        .lines()
        // filter out lines that cannot be read
        .filter_map(Result::ok)
        // skip the specified number of lines if the skip option is set
        .skip(settings.skip.unwrap_or(0))
        // take only the specified number of lines if the take option is set
        .take(settings.take.unwrap_or(usize::MAX))
    {
        // split by the specified delimiter
        let fields: Vec<&str> = line.split(&settings.delimiter).collect();

        // for now, we always validate the number of fields against the mapping if the mapping is not greedy
        // TODO: implement --strict option to control this behavior
        let mapping_count = mapping_columns_count(&settings.mapping);
        if let Some(count) = mapping_count {
            let fields_count = fields.len();
            if fields_count != count {
                return Err(format!(
                    "Expected {} fields based on mapping, but got {}: '{}'",
                    count, fields_count, line
                ))?;
            }
        }

        // let's use an iterator to process the fields according to the mapping, and consume columns as we go
        let mut fields_iterator = fields.into_iter();

        // the final output fields that will be displayed
        let mut selected_fields: Vec<(String, SelectedField)> = Vec::new();

        // FIXME: even this this match works as expected, it is hard to read and has redundant code
        for mapping in &settings.mapping {
            match mapping {
                FieldMap::One { name } => {
                    if let Some(field) = fields_iterator.next() {
                        // we have to do this check for each field and fieldmap type, because this column might
                        // not make it into the output
                        if selected_field_names.contains(name.as_str()) {
                            selected_fields
                                .push((name.clone(), SelectedField::One(field.to_string())));
                        }
                    } else {
                        return Err(format!(
                            "Expected more fields for mapping '{}', but got fewer: '{}'",
                            name, line
                        ))?;
                    }
                }
                FieldMap::Some { name, colspan } => {
                    let mut values = Vec::new();
                    for _ in 0..*colspan {
                        if let Some(field) = fields_iterator.next() {
                            if selected_field_names.contains(name.as_str()) {
                                values.push(field.to_string());
                            }
                        } else {
                            return Err(format!(
                                "Expected more fields for mapping '{}', but got fewer: '{}'",
                                name, line
                            ))?;
                        }
                    }
                    if !values.is_empty() {
                        selected_fields.push((name.clone(), SelectedField::Some(values)));
                    }
                }
                FieldMap::Greedy { name } => {
                    let remaining_fields: Vec<String> =
                        fields_iterator.map(|f| f.to_string()).collect();

                    if selected_field_names.contains(name.as_str()) {
                        selected_fields.push((name.clone(), SelectedField::Some(remaining_fields)));
                    }

                    // since greedy consumes all remaining fields, and ".map()" will exhaust the iterator,
                    // we have to do this explicitly to satisfy the borrow checker
                    break;
                }
            }
        }

        if settings.json {
            serialize_json_object(&mut writer, &selected_fields)?;
            writer.write_all(b"\n")?;
        } else {
            // colspan and greedy fields will be joined by the output_greedy_delimiter,
            // and then all fields will be joined by the output_delimiter
            let output_fields: Vec<String> = selected_fields
                .into_iter()
                .map(|(_, field)| match field {
                    SelectedField::One(val) => val,
                    SelectedField::Some(vals) => vals.join(&settings.output_greedy_delimiter),
                })
                .collect();

            writer.write_all(output_fields.join(&settings.output_delimiter).as_bytes())?;
            writer.write_all(b"\n")?;
        }
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

fn serialize_json_object(
    writer: &mut impl Write,
    fields: &[(String, SelectedField)],
) -> AnyResult<()> {
    writer.write_all(b"{")?;

    for (i, (name, value)) in fields.iter().enumerate() {
        if i > 0 {
            writer.write_all(b", ")?;
        }

        writer.write_all(b"\"")?;
        writer.write_all(name.as_bytes())?;
        writer.write_all(b"\": ")?;

        match value {
            SelectedField::One(value) => {
                writer.write_all(b"\"")?;
                writer.write_all(value.as_bytes())?;
                writer.write_all(b"\"")?;
            }
            SelectedField::Some(values) => {
                writer.write_all(b"[")?;
                for (j, val) in values.iter().enumerate() {
                    if j > 0 {
                        writer.write_all(b", ")?;
                    }
                    writer.write_all(b"\"")?;
                    writer.write_all(val.as_bytes())?;
                    writer.write_all(b"\"")?;
                }
                writer.write_all(b"]")?;
            }
        }
    }
    writer.write_all(b"}")?;

    Ok(())
}
