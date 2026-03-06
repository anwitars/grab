use crate::{options::AppOptions, types::AnyResult};
use std::io::{BufRead, BufReader};

/// Represents the source of the input stream, either from standard input or a file.
#[derive(Debug)]
pub enum StreamSource {
    Stdin(std::io::StdinLock<'static>),
    File(BufReader<std::fs::File>),
}

/// Processes the input line by line according to the provided settings and outputs the results.
pub fn process<R: BufRead>(reader: &mut R, settings: &AppOptions) -> AnyResult<()> {
    for line in reader
        .lines()
        // skip the specified number of lines if the skip option is set
        .skip(settings.skip.unwrap_or(0))
        // filter out lines that cannot be read
        .filter_map(Result::ok)
    {
        // split by the specified delimiter
        let fields: Vec<&str> = line.split(&settings.delimeter).collect();

        if fields.len() != settings.mapping.len() {
            return Err(format!(
                "Field count mismatch: expected {}, got {}",
                settings.mapping.len(),
                fields.len()
            )
            .into());
        }

        let selected_fields: Vec<_> = settings
            .mapping
            .iter()
            // pair each mapping key with the corresponding field value
            .zip(fields.iter())
            // if the select option is set, filter out fields that are not in the select list
            .filter(|(key, _)| {
                settings
                    .select
                    .as_ref()
                    .map_or(true, |select| select.contains(key))
            })
            .collect();

        if settings.json {
            // create a JSON object from the selected fields
            let json_object = selected_fields
                .into_iter()
                .map(|(key, value)| (key.clone(), serde_json::Value::String(value.to_string())))
                .collect::<serde_json::Map<_, _>>();

            println!(
                "{}",
                serde_json::to_string(&json_object)
                    .map_err(|e| format!("Failed to serialize JSON: {}", e))?
            );
        } else {
            // create a delimited string from the selected fields
            let output = selected_fields
                .into_iter()
                .map(|(_, value)| value.to_string())
                .collect::<Vec<_>>()
                .join(&settings.output_delimeter);

            println!("{}", output);
        }
    }

    Ok(())
}
