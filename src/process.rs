use crate::{
    options::{AppOptions, FieldMap},
    types::AnyResult,
};
use std::{
    collections::HashSet,
    io::{BufRead, BufReader},
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
    let selected_field_names: HashSet<&str> = settings
        .select
        .as_ref()
        .map(|s| s.iter().map(|f| f.as_str()).collect())
        .unwrap_or_else(|| settings.mapping.iter().map(|m| m.name()).collect());

    for line in reader
        .lines()
        // skip the specified number of lines if the skip option is set
        .skip(settings.skip.unwrap_or(0))
        // filter out lines that cannot be read
        .filter_map(Result::ok)
    {
        // split by the specified delimiter
        let fields: Vec<&str> = line.split(&settings.delimiter).collect();

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

        let mut fields_iterator = fields.into_iter();
        let mut selected_fields: Vec<(String, SelectedField)> = Vec::new();

        for mapping in &settings.mapping {
            match mapping {
                FieldMap::One { name } => {
                    if let Some(field) = fields_iterator.next() {
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
                    break; // Greedy consumes all remaining fields, so we can stop processing mappings
                }
            }
        }

        if settings.json {
            let json_object: serde_json::Map<String, serde_json::Value> = selected_fields
                .into_iter()
                .map(|(name, field)| {
                    let value = match field {
                        SelectedField::One(val) => serde_json::Value::String(val),
                        SelectedField::Some(vals) => serde_json::Value::Array(
                            vals.into_iter().map(serde_json::Value::String).collect(),
                        ),
                    };
                    (name, value)
                })
                .collect();

            let json_string = serde_json::to_string(&json_object)?;
            println!("{}", json_string);
        } else {
            let output_fields: Vec<String> = selected_fields
                .into_iter()
                .map(|(_, field)| match field {
                    SelectedField::One(val) => val,
                    SelectedField::Some(vals) => vals.join(&settings.output_greedy_delimiter),
                })
                .collect();

            println!("{}", output_fields.join(&settings.output_delimiter));
        }
    }

    Ok(())
}

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
