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

    /// Comma-separated list of field names corresponding to the input columns.
    /// Names must be unique and match the expected field count of the input data unless --loose is enabled.
    ///
    /// A simple mapping example: "name,age,city" means
    /// the first column is "name", the second is "age", and the third is "city".
    ///
    /// Colspan can be used to combine multiple input columns into a single output field.
    /// For example: "name,age,address:2" means "name" is the first column, "age" is the second, and "address"
    /// combines the third and fourth columns into one field using the output-greedy-delimiter.
    /// Example output: "John,30,123 Main St;Apt 4B" if the input was "John,30,123 Main St,Apt 4B".
    ///
    /// Greedy flag can be used to indicate that a field should consume all remaining input columns.
    /// For example: "name,age,rest:g" means "name" is the first column, "age" is the second,
    /// and "rest" consumes all remaining columns.
    /// Example output: "John,30,Extra1;Extra2;Extra3" if the input was "John,30,Extra1,Extra2,Extra3".
    ///
    /// '_' can be used as a placeholder for fields that should be ignored. For example, "name,age,_,city"
    /// means the third column is ignored and not included in the output.
    /// Can be combined with colspan and greedy.
    /// Example output: "John,30,New York" if the input was "John,30,ignored,New York".
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

    /// By default, the tool will error if the number of input fields does not match the mapping.
    /// This flag allows for more lenient parsing, simply not caring about extra or missing fields.
    /// When enabled, it might result in extra fields being ignored or missing fields being filled with empty strings,
    /// depending on the mapping configuration.
    /// Use only if the input data is known to be inconsistent and you want to extract whatever can be extracted without
    /// strict validation.
    #[clap(long)]
    pub loose: bool,

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
