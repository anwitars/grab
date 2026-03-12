use std::collections::HashSet;
use std::str::FromStr;

use crate::cli::Cli;
use crate::types::Delimiter;

#[derive(Debug, thiserror::Error)]
#[cfg_attr(test, derive(PartialEq))]
pub enum FieldMapParseError {
    #[error("Invalid mapping format: '{0}'")]
    InvalidFormat(String),
    #[error("Invalid colspan value '{mapping}': {source}")]
    InvalidColspan {
        mapping: String,
        source: std::num::ParseIntError,
    },
}

#[derive(Debug, thiserror::Error)]
#[cfg_attr(test, derive(PartialEq))]
pub enum AppOptionsValidationError {
    #[error("Mapping cannot be empty")]
    EmptyMapping,
    #[error("Mapping field name cannot be empty at position {position}")]
    EmptyMappingName { position: usize },
    #[error("Duplicate field in mapping: {0}")]
    DuplicateMappingField(String),
    #[error("Colspan must be greater than 0 for mapping '{0}'")]
    ColspanBelowOne(String),
    #[error("{0}")]
    ColspanParseError(#[from] FieldMapParseError),
    #[error("Greedy field must be the last in the mapping: '{0}'")]
    GreedyNotLast(String),
    #[error("Select cannot be empty if provided")]
    EmptySelect,
    #[error("Select field name cannot be empty at position {position}")]
    EmptySelectName { position: usize },
    #[error(
        "Select cannot contain '_' as it is reserved for unmapped fields, found at position {position}"
    )]
    SelectContainsPlaceholder { position: usize },
    #[error("Select field '{field}' not found in mapping")]
    SelectFieldNotInMapping { field: String },
    #[error("Duplicate field in select: {0}")]
    DuplicateSelectField(String),
}

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

    pub fn is_placeholder(&self) -> bool {
        self.name() == "_"
    }
}

impl FromStr for FieldMap {
    type Err = FieldMapParseError;

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
                    let colspan = span_part.parse::<usize>().map_err(|e| {
                        FieldMapParseError::InvalidColspan {
                            mapping: span_part.to_string(),
                            source: e,
                        }
                    })?;
                    Ok(FieldMap::Some { name, colspan })
                }
            }
            _ => Err(FieldMapParseError::InvalidFormat(value.to_string())),
        }
    }
}

/// Parsed and validated application options derived from command-line arguments.
#[derive(Debug)]
pub struct AppOptions {
    pub mapping: Vec<FieldMap>,
    pub select: Option<Vec<String>>,
    pub skip: Option<usize>,
    pub take: Option<usize>,
    pub loose: bool,
    pub delimiter: Delimiter,
    pub output_delimiter: String,
    pub output_greedy_delimiter: String,
    pub json: bool,
}

impl AppOptions {
    /// Validates the mapping options
    fn validate_mapping(&self) -> Result<(), AppOptionsValidationError> {
        if self.mapping.is_empty() {
            return Err(AppOptionsValidationError::EmptyMapping);
        }

        let mut seen = HashSet::new();
        for (i, m) in self.mapping.iter().enumerate() {
            let name = m.name();

            if name.is_empty() {
                return Err(AppOptionsValidationError::EmptyMappingName { position: i + 1 });
            }

            if name != "_" && !seen.insert(name) {
                return Err(AppOptionsValidationError::DuplicateMappingField(
                    name.to_string(),
                ));
            }

            match m {
                FieldMap::Some { colspan, .. } => {
                    if *colspan == 0 {
                        return Err(AppOptionsValidationError::ColspanBelowOne(name.to_string()));
                    }
                }
                FieldMap::Greedy { .. } => {
                    if i != self.mapping.len() - 1 {
                        return Err(AppOptionsValidationError::GreedyNotLast(name.to_string()));
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Validates the select options
    fn validate_select(&self) -> Result<(), AppOptionsValidationError> {
        if let Some(ref select) = self.select {
            if select.is_empty() {
                return Err(AppOptionsValidationError::EmptySelect);
            }

            for (i, s) in select.iter().enumerate() {
                if s.trim().is_empty() {
                    return Err(AppOptionsValidationError::EmptySelectName { position: i + 1 });
                }
            }

            let mapping_set: HashSet<_> = self
                .mapping
                .iter()
                .map(|m| m.name())
                .filter(|name| name != &"_")
                .collect();

            for (i, s) in select.iter().enumerate() {
                if s == "_" {
                    return Err(AppOptionsValidationError::SelectContainsPlaceholder {
                        position: i + 1,
                    });
                }
                if !mapping_set.contains(s.as_str()) {
                    return Err(AppOptionsValidationError::SelectFieldNotInMapping {
                        field: s.clone(),
                    });
                }
            }

            let mut seen = HashSet::new();
            for s in select {
                if !seen.insert(s) {
                    return Err(AppOptionsValidationError::DuplicateSelectField(s.clone()));
                }
            }
        }

        Ok(())
    }

    /// Runs all validation checks on the options.
    pub fn validate(&self) -> Result<(), AppOptionsValidationError> {
        self.validate_mapping()?;
        self.validate_select()?;
        Ok(())
    }

    pub fn selected_mappings(&self) -> Vec<(&FieldMap, bool)> {
        let selected_fields = self
            .select
            .as_ref()
            .map(|s| s.iter().map(AsRef::<str>::as_ref).collect::<HashSet<_>>());

        self.mapping
            .iter()
            .map(|mapping| {
                let is_selected = if mapping.is_placeholder() {
                    false
                } else {
                    selected_fields
                        .as_ref()
                        .map(|s| s.contains(mapping.name()))
                        .unwrap_or(true)
                };
                (mapping, is_selected)
            })
            .collect()
    }
}

impl TryFrom<Cli> for AppOptions {
    type Error = AppOptionsValidationError;

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
        let loose = cli.loose;
        let delimiter = cli.delimiter;
        let json = cli.json;
        let output_delimiter = cli.output_delimiter;
        let output_greedy_delimiter = cli.output_greedy_delimiter;

        Ok(AppOptions {
            mapping,
            select,
            skip,
            take,
            loose,
            delimiter,
            json,
            output_delimiter,
            output_greedy_delimiter,
        })
    }
}

#[cfg(test)]
impl Default for AppOptions {
    fn default() -> Self {
        AppOptions {
            mapping: Vec::new(),
            select: None,
            skip: None,
            take: None,
            loose: false,
            delimiter: Delimiter::Character(b','),
            json: false,
            output_delimiter: ",".to_string(),
            output_greedy_delimiter: ";".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    mod validation {
        use super::*;

        fn from_args(args: &[&str]) -> Result<(), AppOptionsValidationError> {
            let cli = Cli::parse_from(args);
            AppOptions::try_from(cli)
                .expect("Failed to parse options")
                .validate()
        }

        #[test]
        fn empty_mapping_name() {
            assert_eq!(
                from_args(&["wow", "--mapping", "name,,email"]),
                Err(AppOptionsValidationError::EmptyMappingName { position: 2 })
            )
        }

        #[test]
        fn duplicate_mapping_field() {
            assert_eq!(
                from_args(&["wow", "--mapping", "name,age,name"]),
                Err(AppOptionsValidationError::DuplicateMappingField(
                    "name".to_string()
                ))
            )
        }

        #[test]
        fn non_positive_colspan() {
            assert_eq!(
                from_args(&["wow", "--mapping", "name:0,age,email"]),
                Err(AppOptionsValidationError::ColspanBelowOne(
                    "name".to_string()
                ))
            )
        }

        #[test]
        fn greedy_not_last() {
            assert_eq!(
                from_args(&["wow", "--mapping", "name,age:g,email"]),
                Err(AppOptionsValidationError::GreedyNotLast("age".to_string()))
            )
        }

        #[test]
        fn empty_select_name() {
            assert_eq!(
                from_args(&[
                    "wow",
                    "--mapping",
                    "name,age,email",
                    "--select",
                    "name,,email",
                ]),
                Err(AppOptionsValidationError::EmptySelectName { position: 2 })
            )
        }

        #[test]
        fn select_contains_placeholder() {
            assert_eq!(
                from_args(&["wow", "--mapping", "name,age,_", "--select", "name,_",]),
                Err(AppOptionsValidationError::SelectContainsPlaceholder { position: 2 })
            )
        }

        #[test]
        fn select_field_not_in_mapping() {
            assert_eq!(
                from_args(&[
                    "wow",
                    "--mapping",
                    "name,age,email",
                    "--select",
                    "name,email,gender",
                ]),
                Err(AppOptionsValidationError::SelectFieldNotInMapping {
                    field: "gender".to_string()
                })
            )
        }

        #[test]
        fn duplicate_select_field() {
            assert_eq!(
                from_args(&[
                    "wow",
                    "--mapping",
                    "name,age,email",
                    "--select",
                    "name,email,name",
                ]),
                Err(AppOptionsValidationError::DuplicateSelectField(
                    "name".to_string()
                ))
            )
        }
    }
}
