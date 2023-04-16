use std::{
    collections::HashMap, thread::{self, ThreadId, JoinHandle}, io,
    sync::{mpsc::{Receiver, Sender, self}, Arc},
    os::unix::net::UnixDatagram, path::PathBuf, ops::SubAssign,
};

use priority_queue::PriorityQueue;

use crate::core::{client_task::ClientTask, limits::{RunningFilters}, monitor::{Monitor, MonitorResult}, messaging::{self, MessageToClient}};

pub type UdSocketClosure = Box<dyn FnOnce() -> () + Send + 'static>;

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
    receiver: Receiver<messaging::MessageToServer>,

    udsocket: Arc<UnixDatagram>,
    udsock_mngr: Option<JoinHandle<UdSocketClosure>>,

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

    pub fn get_sender(&self) -> &Sender<messaging::MessageToServer> {
        &self.sender
    }

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

    pub fn spawn_udsock_mngr(&mut self, thread_name: &str, fun: UdSocketClosure) -> io::Result<()> {
        let udsocket_manager = thread::Builder::new()
            .name(String::from(thread_name))
            .spawn(move || fun)?;

        self.udsock_mngr = Some(udsocket_manager);

        Ok(())
    }

    pub fn accept_task(&mut self, task: ClientTask) -> io::Result<usize> {
        let client_pid = task.client_pid;
        let prio = task.priority;
        self.task_pqueue.push(task, prio);
    
        let destination = self.udsock_dir.join(
                String::from("sdstore_") + &client_pid.to_string() + &".sock"
        );
        let msg_to_client = MessageToClient::Pending;
    
        let bytes = bincode::serialize(&msg_to_client).unwrap();
        self.udsocket.send_to(&bytes, destination)
    }

    pub fn process_task_result(&mut self, result: MonitorResult) -> io::Result<usize> {
            let mut destination: Option<PathBuf> = None;
    
            let msg_to_client = match result {
                Err(_) => MessageToClient::RequestError,
                Ok((thread_id, exit_status)) => {
                    if let Some(monitor) = self.running_tasks.remove(&thread_id) {
                        // Deduct the completed tasks' filter counts from the server's.
                        self.filters_count.sub_assign(&monitor.task.get_transformations());
    
                        destination = Some(self.udsock_dir
                            .join(String::from("sdstore_") + &monitor.task.client_pid.to_string() + &".sock"
                        ));
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
                _ => {todo!()}
            }
    }
}