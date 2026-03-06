use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
};

use clap::Parser;

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
}

#[derive(Debug)]
struct AppSettings {
    mapping: Vec<String>,
    select: Option<Vec<String>>,
    skip: Option<usize>,
    delimeter: String,
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

        AppSettings {
            mapping,
            select,
            skip,
            delimeter,
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let source = if let Some(ref file) = cli.file {
        StreamSource::File(BufReader::new(
            std::fs::File::open(file).expect("Failed to open file"),
        ))
    } else {
        StreamSource::Stdin(std::io::stdin().lock())
    };

    let settings = AppSettings::from(cli);

    match source {
        StreamSource::Stdin(mut stdin) => process(&mut stdin, &settings),
        StreamSource::File(mut file) => process(&mut file, &settings),
    }
}

fn process<R: BufRead>(reader: &mut R, settings: &AppSettings) {
    for line in reader.lines().skip(settings.skip.unwrap_or(0)) {
        let line = line.expect("Failed to read line");
        let fields: Vec<&str> = line.split(&settings.delimeter).collect();

        let selected_fields = match &settings.select {
            None => fields,
            Some(selected) => fields
                .iter()
                .zip(&settings.mapping)
                .filter(|(_, f)| selected.contains(f))
                .map(|(field, _)| *field)
                .collect(),
        };

        // Here you would implement the logic to map and select fields based on settings.mapping and settings.select
        println!("{}", selected_fields.join(","));
    }
}
