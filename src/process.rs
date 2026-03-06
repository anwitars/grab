use crate::{options::AppOptions, types::AnyResult};
use std::io::{BufRead, BufReader};

#[derive(Debug)]
pub enum StreamSource {
    Stdin(std::io::StdinLock<'static>),
    File(BufReader<std::fs::File>),
}

pub fn process<R: BufRead>(reader: &mut R, settings: &AppOptions) -> AnyResult<()> {
    for line in reader
        .lines()
        .skip(settings.skip.unwrap_or(0))
        .filter_map(Result::ok)
    {
        let fields: Vec<&str> = line.split(&settings.delimeter).collect();

        let selected_fields: Vec<_> = settings
            .mapping
            .iter()
            .zip(fields.iter())
            .filter(|(key, _)| {
                settings
                    .select
                    .as_ref()
                    .map_or(true, |select| select.contains(key))
            })
            .collect();

        if settings.json {
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
            let output = selected_fields
                .into_iter()
                .map(|(_, value)| value.to_string())
                .collect::<Vec<_>>()
                .join(&settings.delimeter);

            println!("{}", output);
        }
    }

    Ok(())
}
