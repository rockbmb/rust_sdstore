use crate::client;

use std::{process::{Command, Stdio, ExitStatus}, path::Path, fs, io};

#[derive(Debug)]
pub enum MonitorError {
    NoTransformationsGiven,
    InputFileError(io::Error),
    OutputFileError(io::Error),
    FilterFailure(String, io::Error),
    PipelineFailure(io::Error)
}

/// Convenience function to report an error during the execution of a
/// filter pipeline.
///
/// The arguments it receives are:
/// * `cmd: &Command`: The offending filter
/// * `ix: usize`: The index of the filter in the task list.
/// * `err_constructor: impl Fn(&'static str) -> MonitorError`: The `MonitorError`
///   variant constructor used to wrap the error message
fn err_msg(
    cmd: &Command,
    ix: usize,
    err: io::Error,
) -> MonitorError {
    let err_msg = format!("filter at position {ix}: {:?}", cmd.get_program().to_os_string());
    MonitorError::FilterFailure(err_msg, err)
}

/// Given a client's task and the path to the transformations the server was given
/// when it began execution, run the tasks to completion.
///
/// Care is taken to create the necessary output file, and route the child processes'
/// pipes in the correct order, so that each filter in the pipeline can pipe its output
/// into the next filter's `STDIN`.
pub fn start_pipeline_monitor(
    task: client::Task,
    transformations_path: &Path
) -> Result<ExitStatus, MonitorError> {
    let transfs_execs = task.get_transformations()
        .iter()
        .map(|filter| transformations_path.join(filter.to_string()))
        .collect::<Vec<_>>();

    let input_fd = fs::File::options()
        .read(true)
        .open(task.input_filepath())
        .map_err(MonitorError::InputFileError)?;
    let output_fd = fs::File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .open(task.output_filepath())
        .map_err(MonitorError::OutputFileError)?;

    if transfs_execs.is_empty() {
        return Err(MonitorError::NoTransformationsGiven)
    }

    let mut transformations: Vec<Command> = Vec::new();
    for transformation in transfs_execs.iter() {
        transformations.push(Command::new(transformation));
    }
    // The first filter in the pipeline must read from the file in the client's request
    transformations.first_mut().map(|c| c.stdin(input_fd));
    // The last filter writes to the created output file.
    transformations.last_mut().map(|c| c.stdout(output_fd));

    execute_pipeline(transformations)
}

/// Helper function for [`start_pipeline_monitor`].
///
/// The `sliding_windows` crate is used to iterate over pairs of commands in
/// a sliding window across the entire pipeline, arranging the input and output
/// file descriptors as necessary.
fn execute_pipeline(mut transformations: Vec<Command>) -> Result<ExitStatus, MonitorError> {
    let mut prev_command: Option<&mut Command> = None;

    for (ix, curr_filter) in
        transformations
            .iter_mut()
            .enumerate()
    {
        if let Some(prev_filter) = prev_command {
            let prev_proc = match prev_filter.spawn() {
                Err(err) => return Err(err_msg(&prev_filter, ix, err)),
                Ok(res) => res
            };
            let prev_stdout = match prev_proc.stdout {
                None => {
                    let err = io::ErrorKind::NotFound.into();
                    return Err(err_msg(&prev_filter, ix, err))
                },
                Some(t) => t
            };
            curr_filter.stdin(Stdio::from(prev_stdout));
        }

        prev_command = Some(curr_filter);
    }

    prev_command.unwrap().status().map_err(MonitorError::PipelineFailure)
}