use std::collections::HashSet;

use crate::{cli::Cli, types::AnyResult};

/// Parsed and validated application options derived from command-line arguments.
#[derive(Debug)]
pub struct AppOptions {
    pub mapping: Vec<String>,
    pub select: Option<Vec<String>>,
    pub skip: Option<usize>,
    pub delimeter: String,
    pub output_delimeter: String,
    pub json: bool,
}

impl AppOptions {
    /// Validates the mapping options
    fn validate_mapping(&self) -> AnyResult<()> {
        if self.mapping.is_empty() {
            return Err("Mapping cannot be empty".into());
        }

        if self.mapping.iter().any(|m| m.trim().is_empty()) {
            return Err("Mapping cannot contain empty fields".into());
        }

        let mut seen = HashSet::new();
        for m in &self.mapping {
            if !seen.insert(m) {
                return Err(format!("Duplicate field in mapping: {}", m).into());
            }
        }

        Ok(())
    }

    /// Validates the select options
    fn validate_select(&self) -> AnyResult<()> {
        if let Some(ref select) = self.select {
            if select.is_empty() {
                return Err("Select cannot be empty if provided".into());
            }

            if select.iter().any(|s| s.trim().is_empty()) {
                return Err("Select cannot contain empty fields".into());
            }

            let mapping_set: HashSet<_> = self.mapping.iter().collect();
            for s in select {
                if !mapping_set.contains(s) {
                    return Err(format!("Select field '{}' not found in mapping", s).into());
                }
            }

            let mut seen = HashSet::new();
            for s in select {
                if !seen.insert(s) {
                    return Err(format!("Duplicate field in select: {}", s).into());
                }
            }
        }

        Ok(())
    }

    /// Runs all validation checks on the options.
    pub fn validate(&self) -> AnyResult<()> {
        self.validate_mapping()?;
        self.validate_select()?;
        Ok(())
    }
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
        let output_delimeter = cli.output_delimeter;

        AppOptions {
            mapping,
            select,
            skip,
            delimeter,
            json,
            output_delimeter,
        }
    }
}
