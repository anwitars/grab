use std::collections::HashSet;
use std::str::FromStr;

use crate::{cli::Cli, types::AnyResult};

/// Defines the user's intent for how to map input fields to output fields,
/// including support for greedy and multi-field mappings.
#[derive(Debug)]
pub enum FieldMap {
    /// Maps all remaining input fields to a single output field, joining them with the greedy delimiter.
    /// Can only be used as the last mapping in the list.
    Greedy { name: String },
    /// Maps multiple input fields to a single output field, with a specified colspan.
    Some { name: String, colspan: usize },
    /// Maps a single input field to an output field.
    One { name: String },
}

impl FieldMap {
    pub fn name(&self) -> &str {
        match self {
            FieldMap::Greedy { name } | FieldMap::Some { name, .. } | FieldMap::One { name } => {
                name
            }
        }
    }
}

impl FromStr for FieldMap {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = value.split(':').collect();
        match parts.len() {
            1 => Ok(FieldMap::One {
                name: parts[0].trim().to_string(),
            }),
            2 => {
                let name = parts[0].trim().to_string();
                let span_part = parts[1].trim();

                if span_part == "g" {
                    Ok(FieldMap::Greedy { name })
                } else {
                    let colspan = span_part
                        .parse::<usize>()
                        .map_err(|e| format!("Invalid colspan value '{}': {}", span_part, e))?;
                    Ok(FieldMap::Some { name, colspan })
                }
            }
            _ => Err(format!("Invalid mapping format: '{}'", value)),
        }
    }
}

/// Parsed and validated application options derived from command-line arguments.
#[derive(Debug)]
pub struct AppOptions {
    pub mapping: Vec<FieldMap>,
    pub select: Option<HashSet<String>>,
    pub skip: Option<usize>,
    pub take: Option<usize>,
    pub delimiter: String,
    pub output_delimiter: String,
    pub output_greedy_delimiter: String,
    pub json: bool,
}

impl AppOptions {
    /// Validates the mapping options
    fn validate_mapping(&self) -> AnyResult<()> {
        if self.mapping.is_empty() {
            return Err("Mapping cannot be empty".into());
        }

        let mut seen = HashSet::new();
        for (i, m) in self.mapping.iter().enumerate() {
            let name = m.name();

            if name.is_empty() {
                return Err(format!("Mapping field name cannot be empty at position {}", i).into());
            }

            if name != "_" && !seen.insert(name) {
                return Err(format!("Duplicate field in mapping: {}", name).into());
            }

            match m {
                FieldMap::Some { colspan, .. } => {
                    if *colspan == 0 {
                        return Err(format!(
                            "Colspan must be greater than 0 in mapping at position {}",
                            i
                        )
                        .into());
                    }
                }
                FieldMap::Greedy { .. } => {
                    if i != self.mapping.len() - 1 {
                        return Err(format!(
                            "Greedy field must be the last in the mapping, found at position {}",
                            i
                        )
                        .into());
                    }
                }
                _ => {}
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

            let mapping_set: HashSet<_> = self
                .mapping
                .iter()
                .map(|m| m.name())
                .filter(|name| name != &"_")
                .collect();

            for s in select {
                if s == "_" {
                    return Err(
                        "Select cannot contain '_' as it is reserved for unmapped fields".into(),
                    );
                }
                if !mapping_set.contains(s.as_str()) {
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

impl TryFrom<Cli> for AppOptions {
    type Error = String;

    fn try_from(cli: Cli) -> Result<Self, Self::Error> {
        let mapping = cli
            .mapping
            .split(',')
            .map(|s| s.parse())
            .collect::<Result<Vec<_>, _>>()?;

        let select = cli
            .select
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect());

        let skip = cli.skip;
        let take = cli.take;
        let delimiter = cli.delimiter;
        let json = cli.json;
        let output_delimiter = cli.output_delimiter;
        let output_greedy_delimiter = cli.output_greedy_delimiter;

        Ok(AppOptions {
            mapping,
            select,
            skip,
            take,
            delimiter,
            json,
            output_delimiter,
            output_greedy_delimiter,
        })
    }
}
