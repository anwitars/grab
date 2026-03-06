use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
};

use clap::Parser;

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Parser)]
struct Cli {
    file: Option<PathBuf>,

    #[clap(short, long)]
    mapping: String,

    #[clap(short, long)]
    select: Option<String>,

    #[clap(long)]
    skip: Option<usize>,

    #[clap(short, long, default_value = ",")]
    delimeter: String,

    #[clap(long)]
    json: bool,
}

#[derive(Debug)]
struct AppSettings {
    mapping: Vec<String>,
    select: Option<Vec<String>>,
    skip: Option<usize>,
    delimeter: String,
    json: bool,
}

#[derive(Debug)]
enum StreamSource {
    Stdin(std::io::StdinLock<'static>),
    File(BufReader<std::fs::File>),
}

impl From<Cli> for AppSettings {
    fn from(cli: Cli) -> Self {
        let mapping = cli
            .mapping
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        let select = cli
            .select
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect());
        let skip = cli.skip;
        let delimeter = cli.delimeter;
        let json = cli.json;

        AppSettings {
            mapping,
            select,
            skip,
            delimeter,
            json,
        }
    }
}

fn main() -> AnyResult<()> {
    let cli = Cli::parse();

    let source = if let Some(ref file) = cli.file {
        StreamSource::File(BufReader::new(std::fs::File::open(file)?))
    } else {
        StreamSource::Stdin(std::io::stdin().lock())
    };

    let settings = AppSettings::from(cli);

    match source {
        StreamSource::Stdin(mut stdin) => process(&mut stdin, &settings)?,
        StreamSource::File(mut file) => process(&mut file, &settings)?,
    }

    Ok(())
}

fn process<R: BufRead>(reader: &mut R, settings: &AppSettings) -> AnyResult<()> {
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
