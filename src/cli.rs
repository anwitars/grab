use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct Cli {
    pub file: Option<PathBuf>,

    #[clap(short, long)]
    pub mapping: String,

    #[clap(short, long)]
    pub select: Option<String>,

    #[clap(long)]
    pub skip: Option<usize>,

    #[clap(short, long, default_value = ",")]
    pub delimeter: String,

    #[clap(short, long, default_value = ",", conflicts_with = "json")]
    pub output_delimeter: String,

    #[clap(long)]
    pub json: bool,
}
