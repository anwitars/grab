use clap::Parser;
use std::{path::PathBuf, str::FromStr};

const DEFAULT_WRITER_BUFFER_SIZE: WriterBufferSize = WriterBufferSize(64 * 1024); // 64KB

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

    /// Enable lenient mode: disables column count validation. Extra fields are ignored; missing fields default to empty
    /// strings.
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

    /// Set the buffer size for the output writer in bytes.
    #[clap(short, long, default_value_t = DEFAULT_WRITER_BUFFER_SIZE)]
    pub buffer: WriterBufferSize,
}

/// Wrapper type for writer buffer size to allow parsing from command-line arguments.
/// Example format: "512K", "128B", "1M". Supports suffixes B, K, M for bytes, kilobytes, and megabytes respectively.
#[derive(Debug, Clone)]
pub struct WriterBufferSize(usize);

impl WriterBufferSize {
    pub fn size(&self) -> usize {
        self.0
    }
}

impl FromStr for WriterBufferSize {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_lowercase();
        if s.is_empty() {
            return Err("Buffer size cannot be empty".into());
        }

        // Check the last character for the unit
        let last_char = s.chars().last().unwrap();
        let multiplier = match last_char {
            'b' => 1,
            'k' => 1024,
            'm' => 1024 * 1024,
            'g' => 1024 * 1024 * 1024, // Might as well add G!
            '0'..='9' => 1,            // Default to bytes if it's a number
            _ => {
                return Err(format!(
                    "Invalid suffix '{}'. Use b, k, m, or g.",
                    last_char
                ));
            }
        };

        let number_part = if last_char.is_alphabetic() {
            &s[..s.len() - 1]
        } else {
            &s
        };

        let number: usize = number_part
            .trim()
            .parse()
            .map_err(|_| format!("'{}' is not a valid number", number_part))?;

        Ok(WriterBufferSize(number * multiplier))
    }
}

impl std::fmt::Display for WriterBufferSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let size = self.0;
        if size >= 1024 * 1024 {
            write!(f, "{}M", size / (1024 * 1024))
        } else if size >= 1024 {
            write!(f, "{}K", size / 1024)
        } else {
            write!(f, "{}B", size)
        }
    }
}
