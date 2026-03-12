use crate::fields::tokenizer::{CsvFieldTokenizer, FieldTokenizer, WhitespaceFieldTokenizer};
use crate::options::AppOptions;
use crate::process::{StreamSource, process};
use crate::types::AnyResult;
use clap::Parser;
use cli::Cli;
use std::io::{BufReader, BufWriter};

mod cli;
mod error;
mod fields;
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

    let mut reader: Box<dyn FieldTokenizer> = match settings.delimiter {
        crate::types::Delimiter::Whitespace => {
            Box::new(WhitespaceFieldTokenizer::new(source.reader()))
        }
        crate::types::Delimiter::Character(delimiter) => {
            Box::new(CsvFieldTokenizer::new(source.reader(), delimiter))
        }
    };

    process(&mut *reader, &mut writer, &settings)?;

    Ok(())
}
