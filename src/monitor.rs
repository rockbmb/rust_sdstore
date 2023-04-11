use std::{path::Path, fs::{self, File}, io};

use subprocess::{Exec, Pipeline, PopenError, ExitStatus};

use crate::client;



/// A selection of the errors a monitor may enconter during a pipeline's execution.
#[derive(Debug)]
pub enum MonitorError {
    /// When executing the pipeline, it had 0 commands. This isn't supposed to happen
    /// as the server checks this before running a pipeline.
    NoTransformationsGiven,
    /// A problem opening/reading the input file.
    InputFileError(io::Error),
    /// A problem creating/opening the output file.
    OutputFileError(io::Error),
    /// A general error may occurrs after `wait`ing for the process responsible for the last
    /// step in the pipeline to finish.
    PipelineFailure(PopenError)
}

/* /// Convenience function to report an error during the execution of a
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
    let err_msg = format!("filter at position {ix}: {:?}", cmd.get_program().to_ascii_lowercase());
    MonitorError::FilterFailure(err_msg, err)
} */

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

    let mut transformations: Vec<Exec> = Vec::new();
    for transf in transfs_execs.iter() {
        transformations.push(Exec::cmd(transf));
    }

    execute_pipeline(transformations, input_fd, output_fd)
}

/// Helper function for [`start_pipeline_monitor`].
fn execute_pipeline(mut transformations: Vec<Exec>, input_fd: File, output_fd: File) -> Result<ExitStatus, MonitorError> {
    if transformations.len() > 1 {
        let mut pipeline = Pipeline::from_exec_iter(transformations);
        // The first filter in the pipeline must read from the file in the client's request
        pipeline = pipeline.stdin(input_fd);
        // The last filter writes to the created output file.
        pipeline = pipeline.stdout(output_fd);
    
        pipeline.join().map_err(|err| {
            MonitorError::PipelineFailure(err)
        })
    } else {
        let mut exec = transformations.remove(0);
        // The first filter in the pipeline must read from the file in the client's request
        exec = exec.stdin(input_fd);
        // The last filter writes to the created output file.
        exec = exec.stdout(output_fd);

        exec.join().map_err(|err| {
            MonitorError::PipelineFailure(err)
        })
    }
}