use std::{hash::Hash, path::{Path, PathBuf}, num::ParseIntError, str::FromStr};

use serde::{Serialize, Deserialize};

use super::filter::{Filter, FilterParseError};

/// This `struct` represents a request, to the `sdstore` server, to apply a sequence
/// of filters to the input file, thereby producing the output at the specified location.
///
/// The PID of the client process is part of this structure since the server must know from
/// the request came, and to whom send information of when the requested task has begun execution
/// or has completed.
///
/// The `PartialOrd` and `Ord` implementations are needed for insertion into the server's
/// task priority queue.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Hash)]
pub struct ClientTask {
    pub client_pid: u32,
    pub priority: usize,
    input: PathBuf,
    output: PathBuf,
    pub transformations: Vec<Filter>
}

impl ClientTask {
    pub fn new(
        client_pid: u32,
        priority: usize,
        input: PathBuf,
        output: PathBuf,
        transformations: Vec<Filter>) -> Self
    {
        ClientTask {
            client_pid,
            priority,
            input,
            output,
            transformations
        }
    }
}

impl PartialOrd for ClientTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.priority.cmp(&other.priority))
    }
}

impl Ord for ClientTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }
}

/// When parsing the client's request from the CLI to turn it into a [`Task`], this enum encodes
/// the errors that may occur.
#[derive(Debug, PartialEq, Eq)]
pub enum TaskParseError {
    InvalidPriority(ParseIntError),
    NoPriorityProvided,
    InvalidInputOutputPaths,
    NoFiltersProvided,
    InvalidFilterProvided(FilterParseError)
}

impl ClientTask {
    /// Build a [`Task`] from `main`'s `args` iterator, parsing the user's input
    /// to construct a request to the server.
    ///
    /// This method is meant to be called from the homologous [`ClientRequest`]
    /// method, and not by itself.
    pub fn build(
        mut args: impl Iterator<Item = String>,
        client_pid: u32
    ) -> Result<Self, TaskParseError> {
        // A task is only ever parsed from the CLI as part of a client
        // request, so the `args` iterator here has already been moved to
        // the priority section of the request.

        let priority: usize = match args.next() {
            None => return Err(TaskParseError::NoPriorityProvided),
            Some(prio) => {
                match prio.trim().parse() {
                    Err(err) => return Err(TaskParseError::InvalidPriority(err)),
                    Ok(p) => p
                }
            }
        };

        let (input, output) = match (args.next(), args.next()) {
            (None, _) | (_, None) => return Err(TaskParseError::InvalidInputOutputPaths),
            (Some(input_path), Some(output_path)) =>
                (PathBuf::from(input_path), PathBuf::from(output_path))
        };

        let mut transformations: Vec<Filter> = Vec::new();
        for filter in args {
            match Filter::from_str(filter.as_str()) {
                Err(err) => return Err(TaskParseError::InvalidFilterProvided(err)),
                Ok(f) => transformations.push(f),
            }
        }
        if transformations.is_empty() { return Err(TaskParseError::NoFiltersProvided) }

        let task = ClientTask {
            client_pid,
            priority,
            input,
            output,
            transformations
        };
        Ok(task)
    }

    pub fn get_transformations(&self) -> Vec<Filter> {
        self.transformations.clone()
    }

    pub fn input_filepath(&self) -> &Path {
        self.input.as_path()
    }

    pub fn output_filepath(&self) -> &Path {
        self.output.as_path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_parsing_works() {
        let str_filters = vec![
            "nop", "bcompress", "bdecompress", "gcompress", "gdecompress", "encrypt", "decrypt"
        ];
        let expected = vec![
            Filter::Nop ,Filter::Bcompress, Filter::Bdecompress, Filter::Gcompress,
            Filter::Gdecompress, Filter::Encrypt, Filter::Decrypt
        ];

        let actual = str_filters
            .into_iter()
            .map(Filter::from_str)
            .map(Result::unwrap)
            .collect::<Vec<_>>();

        for (expected, actual) in expected.into_iter().zip(actual.into_iter()) {
            assert_eq!(expected, actual);
        }
    }

    #[test]
    fn filter_parsing_fails() {
        let str = "bcompres";
        let expected = FilterParseError("bcompres".to_string());
        let actual = Filter::from_str(str).unwrap_err();

        assert_eq!(expected, actual);
    }
}