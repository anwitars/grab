use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[clap(
    name = "grab",
    version,
    about = "A flexible command-line tool for processing delimited text data."
)]
pub struct Cli {
    /// Input file to read from. If not provided, reads from standard input.
    pub file: Option<PathBuf>,

    /// Comma-separated list of field names corresponding to the input columns. Required.
    #[clap(short, long)]
    pub mapping: String,

    /// Comma-separated list of field names to select from the mapping. If not provided, all fields are selected.
    #[clap(short, long)]
    pub select: Option<String>,

    /// Number of lines to skip from the input before processing.
    #[clap(long)]
    pub skip: Option<usize>,

    /// Number of lines to process after skipping. If not provided, processes all remaining lines.
    #[clap(long)]
    pub take: Option<usize>,

    /// Delimiter used to split the input fields.
    #[clap(short, long, default_value = ",")]
    pub delimiter: String,

    /// Delimiter used to join the output fields. Ignored if JSON output is enabled.
    #[clap(short, long, default_value = ",", conflicts_with = "json")]
    pub output_delimiter: String,

    /// Delimiter used to join fields for greedy mappings. Ignored if JSON output is enabled.
    #[clap(long, default_value = ";", conflicts_with = "json")]
    pub output_greedy_delimiter: String,

    /// Output results in JSON format instead of delimited text. Conflicts with output-delimiter option.
    #[clap(long)]
    pub json: bool,
}
