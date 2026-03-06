use crate::options::AppOptions;
use crate::process::{StreamSource, process};
use crate::types::AnyResult;
use clap::Parser;
use cli::Cli;
use std::io::BufReader;

mod cli;
mod options;
mod process;
mod types;

fn main() -> AnyResult<()> {
    let cli = Cli::parse();

    let source = if let Some(ref file) = cli.file {
        StreamSource::File(BufReader::new(std::fs::File::open(file)?))
    } else {
        StreamSource::Stdin(std::io::stdin().lock())
    };

    let settings = AppOptions::from(cli);

    match source {
        StreamSource::Stdin(mut stdin) => process(&mut stdin, &settings)?,
        StreamSource::File(mut file) => process(&mut file, &settings)?,
    }

    Ok(())
}
