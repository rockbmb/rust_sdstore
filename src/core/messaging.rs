use std::fmt::Display;

use serde::{Serialize, Deserialize};

use super::{
    client_task::{ClientTask, TaskParseError},
    monitor::MonitorResult
};

/// Messages sent by the server to each client to inform it of the stage
/// at which its request is.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum MessageToClient {
    /// The request could not be started
    RequestInitError,
    /// The request could be assigned to a monitor and start execution, but the
    /// exit status of its monitor was that of failure.
    RequestError,
    /// The request has been received, and is pending processing.
    Pending,
    /// The request has been assigned to a `Monitor`, as has begun processing
    Processing,
    /// The request was sucessfully completed
    Concluded((u64, u64))
}

impl Display for MessageToClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::RequestInitError => write!(f, "the request failed to start. check server logs for information"),
            Self::RequestError     => write!(f, "the request started, but failed. check server logs for information"),
            Self::Pending          => write!(f, "pending"),
            Self::Processing       => write!(f, "processing"),
            Self::Concluded((i, o)) => write!(f, "concluded (bytes-input: {}, bytes-output: {})", i, o),
        }
    }
}

pub enum MessageToServer {
    Client(ClientRequest),
    Monitor(MonitorResult)
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
    ProcFile(ClientTask)
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

        let task = match ClientTask::build(args, client_pid) {
            Err(err) => return Err(ClientReqParseError::TaskParseError(err)),
            Ok(t) => t,
        };

        Ok(ClientRequest::ProcFile(task))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::core::{filter::{Filter, FilterParseError}, client_task::{ClientTask, TaskParseError}, messaging::{ClientRequest, ClientReqParseError}};

    #[test]
    fn task_parsing_works() {
        let command = String::from(
            "./sdstore proc-file 5 samples/file-a outputs/file-a-output bcompress nop gcompress encrypt nop"
        );

        let args = command
            .split_ascii_whitespace()
            .map(str::to_string);
        // Move past executable name, and command type

        let task = ClientTask::new(
            0,
            5,
            PathBuf::from("samples/file-a"),
            PathBuf::from("outputs/file-a-output"),
            vec![Filter::Bcompress, Filter::Nop, Filter::Gcompress, Filter::Encrypt, Filter::Nop]
        );

        let mut args1 = args.clone();
        args1.next();
        args1.next();
        assert_eq!(ClientTask::build(args1, 0).unwrap(), task);

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