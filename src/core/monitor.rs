use std::{
    path::PathBuf, fs, io, thread::{self, Thread, ThreadId}, sync::mpsc::Sender,
};

use subprocess::{Exec, Pipeline, PopenError, ExitStatus};

use super::{client_task, messaging};

/// A selection of the errors a monitor may enconter during a pipeline's execution.
#[derive(Debug)]
pub enum MonitorError {
    /// Problem spawning the thread responsible for the pipeline's execution.
    ThreadSpawnError(io::Error),
    /// When executing the pipeline, it had 0 commands. This isn't supposed to happen
    /// as the server checks this before running a pipeline.
    NoTransformationsGiven,
    /// A problem opening/reading the input file.
    InputFileError(io::Error),
    /// A problem creating/opening the output file.
    OutputFileError(io::Error),
    /// A general error may occurrs after `wait`ing for the process responsible for the last
    /// step in the pipeline to finish.
    PipelineFailure(PopenError),

    /// The pipeline finished, but its exit status was not that of success.
    PipelineExitStatusError(ExitStatus),
    /// A problem opening the input file's metadata to obtain its size.
    InputFileMetadataError(io::Error),
    /// A problem opening the output file's metadata to obtain its size.
    OutputFileMetadataError(io::Error),
    /// Failed to inform the server of pipeline completion via the sending end of an `mpsc::channel`
    MpscSenderError,
}

pub struct Monitor {
    /// Client request the monitor is responsible for
    pub task: client_task::ClientTask,
    /// Numbering of the task, provided by the server. For `Display` purposes.
    /// Only assigned after the task begins execution, not after the server receives
    /// and schedules it.
    task_number: usize,
    /// Thread responsible for executing the pipeline
    thread: Thread,
}

/// Information returned by a monitor on a successful return.
///
/// Size of the input and output files in bytes.
pub type MonitorSuccess = (u64, u64);

/// Result type of a monitor. It'll return:
///
/// * the thread ID of the monitor assigned to the task, and
///   * either the `ExitStatus` of the the pipeline and the total of bytes read/written,
///   * or a `MonitorError`.
pub struct MonitorResult {
    pub thread: ThreadId,
    pub result: Result<MonitorSuccess, MonitorError>
}

impl Monitor {
    pub fn build(
        task: client_task::ClientTask,
        task_number: usize,
        transformations_path: PathBuf,
        sender: Sender<messaging::MessageToServer>
    ) -> Result<Self, MonitorError> {
        let task_clone = task.clone();
        let path_clone = transformations_path.clone();
        let thread = match thread::Builder
            ::new()
            .name(format!("Worker-{}", task.client_pid))
            .spawn(move ||
                start_pipeline_monitor(
                    task_clone,
                    path_clone,
                    sender
                ))
            .map(|handle| handle.thread().clone()) {
                Err(err) => return Err(MonitorError::ThreadSpawnError(err)),
                Ok(t) => t
            };

        Ok(Monitor {
            task,
            task_number,
            thread,
        })
    }

    pub fn thread_id(&self) -> ThreadId {
        self.thread.id()
    }
}

/// Given a client's task and the path to the transformations the server was given
/// when it began execution, run the tasks to completion.
///
/// Care is taken to create the necessary output file, and route the child processes'
/// pipes in the correct order, so that each filter in the pipeline can pipe its output
/// into the next filter's `STDIN`.
fn start_pipeline_monitor(
    task: client_task::ClientTask,
    transformations_path: PathBuf,
    sender: Sender<messaging::MessageToServer>
) -> Result<(), MonitorError> {
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
        .create(true)
        .truncate(true)
        .open(task.output_filepath())
        .map_err(MonitorError::OutputFileError)?;

    if transfs_execs.is_empty() {
        return Err(MonitorError::NoTransformationsGiven)
    }

    let mut transformations: Vec<Exec> = Vec::new();
    for transf in transfs_execs.iter() {
        transformations.push(Exec::cmd(transf));
    }

    let result = if transformations.len() == 1 {
        let mut exec = transformations.remove(0);
        // The first and only filter in the pipeline must read from the file in the client's request,
        // and write to the provided file as well.
        exec = exec.stdin(input_fd);
        exec = exec.stdout(output_fd);
        exec.join()
    } else {
        let mut pipeline = Pipeline::from_exec_iter(transformations);
        // The first filter in the pipeline must read from the file in the client's request
        pipeline = pipeline.stdin(input_fd);
        // The last filter writes to the created output file.
        pipeline = pipeline.stdout(output_fd);
    
        pipeline.join()
    }
    .map_err(|err| { MonitorError::PipelineFailure(err) });

    let result = match result {
        Ok(status) if status.success() => {
            let (bytes_in, bytes_out): (u64, u64) = (
                match fs::metadata(task.input_filepath()) {
                    Err(err) => return Err(MonitorError::InputFileMetadataError(err)),
                    Ok(meta) => meta.len()
                },
                match fs::metadata(task.output_filepath()) {
                    Err(err) => return Err(MonitorError::OutputFileMetadataError(err)),
                    Ok(meta) => meta.len()
                },
            );
            Ok((bytes_in, bytes_out))
        },
        Ok(status) => Err(MonitorError::PipelineExitStatusError(status)),
        Err(err) => Err(err)
    };

    let thread = thread::current().id();
    let monitor_result = MonitorResult {
        thread,
        result
    };

    let result = messaging::MessageToServer::Monitor(monitor_result);

    sender.send(result).map_err(|_| MonitorError::MpscSenderError)
}
