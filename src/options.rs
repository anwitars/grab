use crate::cli::Cli;

#[derive(Debug)]
pub struct AppOptions {
    pub mapping: Vec<String>,
    pub select: Option<Vec<String>>,
    pub skip: Option<usize>,
    pub delimeter: String,
    pub json: bool,
}

impl From<Cli> for AppOptions {
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

        AppOptions {
            mapping,
            select,
            skip,
            delimeter,
            json,
        }
    }
}
