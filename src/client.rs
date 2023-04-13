use std::{path::{Path, PathBuf}, num::ParseIntError, str::FromStr};

use serde::{Serialize, Deserialize};

use crate::filter::{Filter, FilterParseError};

/// This `struct` represents a request, to the `sdstore` server, to apply a sequence
/// of filters to the input file, thereby producing the output at the specified location.
///
/// The PID of the client process is part of this structure since the server must know from
/// the request came, and to whom send information of when the requested task has begun execution
/// or has completed.
///
/// The `PartialOrd` and `Ord` implementations are needed for insertion into the server's
/// task priority queue.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Task {
    pub client_pid: u32,
    priority: usize,
    input: PathBuf,
    output: PathBuf,
    transformations: Vec<Filter>
}
impl PartialOrd for Task {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.priority.cmp(&other.priority))
    }
}

impl Ord for Task {
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

impl Task {
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

        let task = Task {
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

/// The kinds of requests a client may make to the server.
///
/// A client can
/// * request that the server inform it of currently running tasks, task limits
///   and pending requests
/// * request the processing of a file with a given priority, with the sequence of
///   filters listed in the request.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum ClientRequest {
    /// Corresponds to `./sdtore status`
    Status,
    /// Corresponds to `./sdstore proc-file <priority> <input-file> <output-file> [filters]`
    ProcFile(Task)
}

/// Enum for errors that may occur while parsing the client's request from the CLI.
#[derive(Debug, PartialEq, Eq)]
pub enum ClientReqParseError {
    IncorrectCommandProvided,
    NoCommandProvided,
    TaskParseError(TaskParseError),
}

impl ClientRequest {
    /// Build a [`ClientRequest`] from `main`'s `args` iterator, parsing the user's input
    /// to construct a request to the server.
    pub fn build(mut args: impl Iterator<Item = String>, client_pid: u32) -> Result<Self, ClientReqParseError> {
        // Move past executable name in args list
        args.next();

        let command = match args.next() {
            Some(arg) => arg,
            None => return Err(ClientReqParseError::NoCommandProvided),
        };

        match command.as_str() {
            "status" => return Ok(Self::Status),
            "proc-file" => {}
            _  => return Err(ClientReqParseError::IncorrectCommandProvided),
        };

        let task = match Task::build(args, client_pid) {
            Err(err) => return Err(ClientReqParseError::TaskParseError(err)),
            Ok(t) => t,
        };

        Ok(ClientRequest::ProcFile(task))
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

    #[test]
    fn task_parsing_works() {
        let command = String::from(
            "./sdstore proc-file 5 samples/file-a  bcompress nop gcompress encrypt nop"
        );

        let args = command
            .split_ascii_whitespace()
            .map(str::to_string);
        // Move past executable name, and command type

        let task = Task {
            client_pid : 0,
            priority : 5,
            input : PathBuf::from("samples/file-a"),
            output : PathBuf::from("outputs/file-a-output"),
            transformations : vec![Filter::Bcompress, Filter::Nop, Filter::Gcompress, Filter::Encrypt, Filter::Nop]
        };

        let mut args1 = args.clone();
        args1.next();
        args1.next();
        assert_eq!(Task::build(args1, 0).unwrap(), task);

        let client_req = ClientRequest::ProcFile(task);
        assert_eq!(ClientRequest::build(args, 0).unwrap(), client_req);
    }

    #[test]
    fn status_parsing_works() {
        let command = String::from("./sdstore status");
        let args = command
            .split_ascii_whitespace()
            .map(str::to_string);

        assert_eq!(ClientRequest::build(args, 0).unwrap(), ClientRequest::Status);
    }

    #[test]
    fn request_parsing_fails1() {
        let command = String::from("./sdstore abcdef");
        let args = command
            .split_ascii_whitespace()
            .map(str::to_string);

            assert_eq!(
                ClientRequest::build(args, 0).unwrap_err(),
                ClientReqParseError::IncorrectCommandProvided
            );
    }

    #[test]
    fn request_parsing_fails2() {
        let command = String::from("./sdstore");
        let args = command
            .split_ascii_whitespace()
            .map(str::to_string);

            assert_eq!(
                ClientRequest::build(args, 0).unwrap_err(),
                ClientReqParseError::NoCommandProvided
            );
    }

    #[test]
    fn task_parsing_fails1() {
        let command = String::from(
            "./sdstore proc-file"
        );
        let args = command
            .split_ascii_whitespace()
            .map(str::to_string);

        assert_eq!(
            ClientRequest::build(args, 0).unwrap_err(),
            ClientReqParseError::TaskParseError(TaskParseError::NoPriorityProvided)
        );
    }

    #[test]
    fn task_parsing_fails2() {
        let command = String::from("./sdstore proc-file 5a");
        let args = command
            .split_ascii_whitespace()
            .map(str::to_string);

        assert!(matches!(
            ClientRequest::build(args, 0).unwrap_err(),
            ClientReqParseError::TaskParseError(TaskParseError::InvalidPriority(_))
        ));
    }

    #[test]
    fn task_parsing_fails3() {
        let command = String::from(
            "./sdstore proc-file 5 samples/file-a"
        );
        let args = command
            .split_ascii_whitespace()
            .map(str::to_string);

        assert_eq!(
            ClientRequest::build(args, 0).unwrap_err(),
            ClientReqParseError::TaskParseError(TaskParseError::InvalidInputOutputPaths)
        );
    }

    #[test]
    fn task_parsing_fails4() {
        let command = String::from(
            "./sdstore proc-file 5 samples/file-a outputs/file-a-output"
        );
        let args = command
            .split_ascii_whitespace()
            .map(str::to_string);

        assert_eq!(
            ClientRequest::build(args, 0).unwrap_err(),
            ClientReqParseError::TaskParseError(TaskParseError::NoFiltersProvided)
        );
    }

    #[test]
    fn task_parsing_fails5() {
        let command = String::from(
            "./sdstore proc-file 5 samples/file-a outputs/file-a-output nopp"
        );
        let args = command
            .split_ascii_whitespace()
            .map(str::to_string);

        let err = ClientReqParseError::TaskParseError(
            TaskParseError::InvalidFilterProvided(
                FilterParseError(String::from("nopp"))
            )
        );

        assert_eq!(ClientRequest::build(args, 0).unwrap_err(), err );
    }
}