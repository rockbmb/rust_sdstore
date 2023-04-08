use serde::{Serialize, Deserialize};

use std::{str::FromStr, path::PathBuf, num::ParseIntError};

/// Enum representing the kinds of filters a client can request be applied
/// to a file.
///
/// For each of these variants, there will be a corresponding `.c` source and
/// executable in the `bin/` folder, in the root of this project.
#[derive(Serialize, Deserialize, Debug)]
pub enum Filter {
    Nop,
    Bcompress,
    Bdecompress,
    Gcompress,
    Gdecompress,
    Encrypt,
    Decrypt
}

#[derive(Debug)]
pub struct FilterParseError(String);

impl FromStr for Filter {
    type Err = FilterParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let res = match s.to_lowercase().as_str() {
            "nop"         => Filter::Nop,
            "bcompress"   => Filter::Bcompress,
            "bdecompress" => Filter::Bdecompress,
            "gcompress"   => Filter::Gcompress,
            "gdecompress" => Filter::Gdecompress,
            "encrypt"     => Filter::Encrypt,
            "decrypt"     => Filter::Decrypt,
            s             => return Err(FilterParseError(s.to_string()))
        };

        Ok(res)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Task {
    client_pid: u32,
    priority: usize,
    input: PathBuf,
    output: PathBuf,
    transformations: Vec<Filter>
}

#[derive(Debug)]
pub enum TaskParseError {
    InvalidPriority(ParseIntError),
    NoPriorityProvided,
    InvalidInputOutputPaths,
    NoFiltersProvided,
    InvalidFilterProvided(FilterParseError)
}

impl Task {
    /// Build a [`ClientRequest`] from `main`'s `args` iterator, parsing the user's input
    /// to construct a request to the server.
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
}

/// The kinds of requests a client may make to the server.
///
/// A client can
/// * request that the server inform it of currently running tasks, task limits
///   and pending requests
/// * request the processing of a file with a given priority, with the sequence of
///   filters listed in the request.
#[derive(Serialize, Deserialize, Debug)]
pub enum ClientRequest {
    Status,
    ProcFile(Task)
}

#[derive(Debug)]
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