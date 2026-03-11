use crate::options::AppOptions;
use crate::process::{StreamSource, process};
use crate::types::AnyResult;
use clap::Parser;
use cli::Cli;
use std::io::{BufReader, BufWriter};

mod cli;
mod error;
mod options;
mod process;
mod types;

fn main() -> AnyResult<()> {
    let cli = Cli::parse();

    // determine the source of the input stream: either a file or standard input.
    let source = if let Some(ref file) = cli.file {
        StreamSource::File(BufReader::new(std::fs::File::open(file)?))
    } else {
        StreamSource::Stdin(std::io::stdin().lock())
    };

    let buffer_size: usize = cli.buffer.size();
    let settings = AppOptions::try_from(cli)?;
    settings.validate()?;

    let mut writer = BufWriter::with_capacity(buffer_size, std::io::stdout());
    match source {
        StreamSource::Stdin(mut stdin) => process(&mut stdin, &mut writer, &settings)?,
        StreamSource::File(mut file) => process(&mut file, &mut writer, &settings)?,
    }

    Ok(())
}
