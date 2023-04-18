use std::{
    collections::HashMap, thread::{self, ThreadId, JoinHandle}, io,
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

/// Type of the closure used to spawn the socket listener.
pub type UdSocketListener = Box<dyn FnOnce() -> () + Send + 'static>;

/// State a server needs to operate and communicate.
///
/// This excludes the config data parsed from the user's CLI input: that data lives in
/// [`ServerConfig`].
pub struct ServerState {
    /// Counter assigned to each task after reception. Useful when reporting the server's
    /// status to a client.
    task_counter: usize,

    /// Priority queue of tasks sent by clients. All tasks must therefore have a `usize`
    /// priority.
    task_pqueue: PriorityQueue<ClientTask, usize>,

    /// Count of all the filters the server is currently running.
    filters_count: RunningFilters,
    /// Association between `ThreadId`s and the `Monitor`s each represents, where a
    /// `Monitor` is responsible for running a pipeline.
    running_tasks: HashMap<ThreadId, Monitor>,

    /// MPSC sender to be given to:
    /// * each monitor in order to communicate pipeline results back to the server.
    /// * the thread listening to the `UnixDatagram` socket, which uses this sender
    ///   to inform the server of new requests.
    ///
    /// The receiving end is on the server's main thread.
    sender: Sender<messaging::MessageToServer>,
    /// Receiving end of the channel used to receive messages from monitors, and from
    /// the unix datagram socket listening thread.
    pub receiver: Receiver<messaging::MessageToServer>,

    /// Unix datagram socket used to receive messages from clients.
    /// The server's main thread doesn't use it, delegating this task to a thread that then
    /// manages reading messages and sending them back to the main thread via an `mpsc::channel`
    /// to take advantage of its static typing guarantees.
    udsocket: Arc<UnixDatagram>,
    /// Handle of the thread spawned to manage the `UnixDatagram` socket.
    ///
    /// TODO
    /// It needs to be stored to allow the graceful termination of the server: as soon as the
    /// main threads receives a e.g. `SIGINTT/SIGTERM`, this thread will be responsible for
    /// closing the socket and freeing resources.
    udsock_mngr: Option<JoinHandle<()>>,

    /// Path to the folder where the server and clients operate from.
    ///
    /// Note:
    /// From hardcoding server and client paths to using named
    /// non-temporary files created manually for server and client sockets, to
    /// assuming both know where to find each other; these are shortcuts - a
    /// serious project would never have this.
    udsock_dir: PathBuf
}

impl ServerState {
    /// Get a new strong reference to the server's unix datagram socket.
    pub fn get_udsocket(&self) -> Arc<UnixDatagram> {
        Arc::clone(&self.udsocket)
    }

    /// Get a new sender of server messages; useful to give to monitors
    /// to communicate results.
    pub fn get_sender(&self) -> Sender<messaging::MessageToServer> {
        self.sender.clone()
    }

    /// Get, increment, the server's task counter, used to number tasks.
    pub fn get_incr_task_counter(&mut self) -> usize {
        let res = self.task_counter;
        self.task_counter += 1;
        res
    }

    /// Given a client's PID, construct the path of its datagram socket.
    ///
    /// Both server and client sockets exist in a directory named `/tmp`
    /// in the root of this project.
    pub fn get_udsock_dest(&self, client_pid: u32) -> PathBuf {
        self.udsock_dir.join(
            String::from("sdstore_") + &client_pid.to_string() + &".sock"
        )
    }

    /// Create a new instance of `ServerState`, assuming an initialized `UnixDatagram`,
    /// and given intended the path to the server's socket,
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

    /// Spawn a thread to manage the unix datagram socket.
    ///
    /// The closure it is spawned with must give it ownership of a new `Arc` to the socket,
    /// and likewise of a cloned `Sender<MessageToServer>`.
    pub fn spawn_udsock_mngr(&mut self, thread_name: &str, fun: UdSocketListener) -> io::Result<()> {
        let udsocket_manager = thread::Builder::new()
            .name(String::from(thread_name))
            .spawn(move || fun())?;

        self.udsock_mngr = Some(udsocket_manager);

        Ok(())
    }

    /// Insert new inbound task in the priority queue, and inform the sending
    /// client that it is now pending.
    pub fn new_task(&mut self, task: ClientTask) -> io::Result<usize> {
        let client_pid = task.client_pid;
        let prio = task.priority;
        self.task_pqueue.push(task, prio);

        let destination = self.get_udsock_dest(client_pid);
        let msg_to_client = MessageToClient::Pending;

        let bytes = bincode::serialize(&msg_to_client).unwrap();
        self.udsocket.send_to(&bytes, destination)
    }

    /// Attempt to remove the highest priority task in the queue.
    ///
    /// For it to be possible, the following is required:
    ///
    /// * That the server has pending tasks in the queue
    /// * That the task that was sucessfully popped can be run, given the server's
    ///   currently running filter count, and the filters required to execute the task.
    ///
    /// 
    /// If this is not possible, return `None`.
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

    /// Begin processing of a task popped from the priority queue.
    ///
    /// This method:
    /// * updates the server's running filter count to reflect the new task's execution
    /// * handles the creation of a monitor responsible for the task,
    /// * indexes it in the server's hashmap or currently running tasks,
    /// * informs the client its task has begun processing
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
                task, task_number, server_config.transformations_path(), sender_clone
            ).unwrap();
            let monitor_id = monitor.thread_id();

            self.running_tasks.insert(monitor.thread_id(), monitor);

            (monitor_id, task_number)
    }

    /// Given the result of a monitor that was responsible for a given task,
    /// process its data and update the server's state accordingly:
    ///
    /// * inform the client of if the task ended in success or failure, and
    /// * update the server's count of currently running filters
    pub fn handle_task_result(&mut self, mon_res: MonitorResult) -> io::Result<usize> {
        let (thread_id, client_pid, result) = mon_res;
        let destination = self.get_udsock_dest(client_pid);

        let msg_to_client = match result {
            Err(_) => MessageToClient::RequestInitError,
            Ok(exit_status) if exit_status.success() => MessageToClient::Concluded,
            Ok(_) => MessageToClient::RequestError
        };

        if let Some(monitor) = self.running_tasks.remove(&thread_id) {
            // Deduct the completed tasks' filter counts from the server's.
            self.filters_count.sub_assign(&monitor.task.get_transformations());
        } else {
            // This would be very odd: there is a thread in the server supposedly running a
            // monitor, but that monitor does not exist.
            panic!();
        }

        // TODO: this unwrap can be handled, at the expense of quite a bit of additional code
        let bytes = bincode::serialize(&msg_to_client).unwrap();
        self.udsocket.send_to(&bytes, destination)
    }
}