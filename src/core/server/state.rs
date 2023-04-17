use std::{
    collections::HashMap, thread::{self, ThreadId, JoinHandle, Thread}, io,
    sync::{mpsc::{Receiver, Sender, self}, Arc},
    os::unix::net::UnixDatagram, path::PathBuf, ops::{SubAssign, AddAssign},
};

use priority_queue::PriorityQueue;

use crate::core::{
    client_task::ClientTask,
    limits::RunningFilters,
    monitor::{Monitor, MonitorResult},
    messaging::{self, MessageToClient}};

use super::config::ServerConfig;

pub type UdSocketListener = Box<dyn FnOnce() -> () + Send + 'static>;

/// State a server needs to operate and communicate.
///
/// This excludes the config data parsed from the user's CLI input: that data lives in
/// [`ServerConfig`].
pub struct ServerState {
    /// Counter assigned to each task after reception. Useful when reporting the server's
    /// status to a client.
    task_counter: usize,
    task_pqueue: PriorityQueue<ClientTask, usize>,

    filters_count: RunningFilters,
    running_tasks: HashMap<ThreadId, Monitor>,

    sender: Sender<messaging::MessageToServer>,
    pub receiver: Receiver<messaging::MessageToServer>,

    udsocket: Arc<UnixDatagram>,
    udsock_mngr: Option<JoinHandle<()>>,

    /// Path to the folder where the server and clients operate from.
    ///
    /// All of it, from hardcoding server and client paths to using named
    /// non-temporary files created manually, and assuming both know where to
    /// find each other are shortcuts - a serious project would never have this.
    udsock_dir: PathBuf
}

impl ServerState {
    pub fn get_udsocket(&self) -> Arc<UnixDatagram> {
        Arc::clone(&self.udsocket)
    }

    pub fn get_sender(&self) -> Sender<messaging::MessageToServer> {
        self.sender.clone()
    }

    pub fn get_incr_task_counter(&mut self) -> usize {
        let res = self.task_counter;
        self.task_counter += 1;
        res
    }

    pub fn get_udsock_dest(&self, client_pid: u32) -> PathBuf {
        self.udsock_dir.join(
            String::from("sdstore_") + &client_pid.to_string() + &".sock"
        )
    }

    /// Create a new instance of `ServerState`, assuming an initialized `UnixDatagram`,
    /// but creating new inter-thread `mpsc::channel`s.
    pub fn new(udsocket: UnixDatagram, udsock_dir: PathBuf) -> Self {
        let (
            sender,
            receiver
        ) = mpsc::channel::<messaging::MessageToServer>();
        let udsocket = Arc::new(udsocket);

        Self {
            task_counter: 0,
            task_pqueue: PriorityQueue::new(),

            filters_count: RunningFilters::default(),
            running_tasks: HashMap::new(),

            sender,
            receiver,

            udsocket,
            udsock_mngr: None,
            udsock_dir
        }
    }

    pub fn spawn_udsock_mngr(&mut self, thread_name: &str, fun: UdSocketListener) -> io::Result<()> {
        let udsocket_manager = thread::Builder::new()
            .name(String::from(thread_name))
            .spawn(move || fun())?;

        self.udsock_mngr = Some(udsocket_manager);

        Ok(())
    }

    pub fn receive_task(&mut self, task: ClientTask) -> io::Result<usize> {
        let client_pid = task.client_pid;
        let prio = task.priority;
        self.task_pqueue.push(task, prio);

        let destination = self.get_udsock_dest(client_pid);
        let msg_to_client = MessageToClient::Pending;

        let bytes = bincode::serialize(&msg_to_client).unwrap();
        self.udsocket.send_to(&bytes, destination)
    }

    pub fn handle_task_result(&mut self, result: MonitorResult) -> io::Result<usize> {
        let mut destination: Option<PathBuf> = None;

        let msg_to_client = match result {
            Err(_) => MessageToClient::RequestError,
            Ok((thread_id, exit_status)) => {
                if let Some(monitor) = self.running_tasks.remove(&thread_id) {
                    // Deduct the completed tasks' filter counts from the server's.
                    self.filters_count.sub_assign(&monitor.task.get_transformations());

                    destination = Some(self.get_udsock_dest(monitor.task.client_pid));
                    if exit_status.success() {
                        MessageToClient::Concluded
                    } else {
                        MessageToClient::RequestError
                    }
                } else {
                    // This would be odd: there is a thread in the server supposedly running a
                    // monitor, but that monitor does not exist.
                    //
                    // TODO: handle this panic, as well as the unwrap below.
                    panic!();
                }
            }
        };

        match destination {
            Some(dest) => {
                // TODO: this unwrap can be handled, at the expense of quite a bit of additional code
                let bytes = bincode::serialize(&msg_to_client).unwrap();
                self.udsocket.send_to(&bytes, dest)
            },
            _ => { panic!() }
        }
    }

    pub fn try_pop_task(&mut self, server_config: &ServerConfig) -> Option<ClientTask> {
        if let Some((task, _)) = self.task_pqueue.peek() {
            if self.filters_count.can_run_pipeline(
                &server_config.filters_config,
                &task.transformations
            ) {
                // Since the loop is only entered if the queue's highest priority element can be
                // peeked into, this unwrap is safe.
                let (task, _) = self.task_pqueue.pop().unwrap();
                return Some(task);
            }
        }

        None
    }

    pub fn process_task(&mut self, server_config: &ServerConfig, task: ClientTask) -> (ThreadId, usize) {
            let destination = self.get_udsock_dest(task.client_pid);
            let msg_to_client = MessageToClient::Processing;
            // TODO: Handle this unwrap
            let bytes = bincode::serialize(&msg_to_client).unwrap();
            // TODO: handle this unwrap
            self.udsocket.send_to(&bytes, destination).unwrap();

            // update server's limits with new task's counts.
            self.filters_count.add_assign(&task.transformations);
            // get and update server's task counter
            let task_number = self.get_incr_task_counter();

            let sender_clone = self.sender.clone();
            // TODO: don't unwrap here
            let monitor = Monitor::build(
                task,
                task_number,
                server_config.transformations_path(),
                sender_clone).unwrap();
            let monitor_id = monitor.thread_id();

            self.running_tasks.insert(monitor.thread_id(), monitor);

            (monitor_id, task_number)
    }
}