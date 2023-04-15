use std::{
    collections::HashMap,
    env, process, fs, io, sync::{mpsc::{self, Sender}, Arc},
    thread::{ThreadId, self}, path::Path, ops::{SubAssign, AddAssign},
};

use interprocess::os::unix::udsocket::{self, UdSocket};
use priority_queue::PriorityQueue;

use rust_sdstore::{
    core::{
        messaging::ClientRequest,
        server_config::{self, ServerConfig}, limits::{self, RunningFilters},
        client_task::ClientTask,
        monitor::{Monitor, MonitorResult}, messaging::{self, MessageToClient}
    }
};

fn udsock_listen(listener: Arc<UdSocket>, sender: mpsc::Sender<messaging::MessageToServer>) {
    // Loop the processing of clients' requests.
    let mut buf = [0; 1024];
    loop {
        let (n, _) = listener.recv(&mut buf).unwrap_or_else(|err| {
            log::error!("Could not read from UdSocket. Error: {:?}", err);
            process::exit(1);
        });
        // TODO: handle this unwrap
        let request: ClientRequest = bincode::deserialize(&buf[..n]).unwrap();

        // TODO: this unwrap needs to be handled
        sender.send(messaging::MessageToServer::Client(request)).unwrap();
    }
}

fn queue_task(task_pqueue: &mut PriorityQueue<ClientTask, usize>, task: ClientTask) {
    let prio = task.priority;
    task_pqueue.push(task, prio);
}

fn accept_task(
    task_pqueue: &mut PriorityQueue<ClientTask, usize>,
    listener: &Arc<UdSocket>,
    client_udsock_path: &Path,
    task: ClientTask
) -> io::Result<usize> {
    let client_pid = task.client_pid;
    queue_task(task_pqueue, task);

    listener
        .set_destination(client_udsock_path.join(
            String::from("sdstore_") + &client_pid.to_string() + &".sock"))?;
    let msg_to_client = MessageToClient::Pending;

    let bytes = bincode::serialize(&msg_to_client).unwrap();
    listener.send(&bytes)
}

fn sunset_task(
    thread_id: ThreadId,
    running_tasks: &mut HashMap<ThreadId, Monitor>
) -> Option<Monitor> {
    running_tasks.remove(&thread_id)
}

fn process_task_result(
    result: MonitorResult,
    running_tasks: &mut HashMap<ThreadId, Monitor>,
    filters_count: &mut RunningFilters,
    listener: &Arc<UdSocket>,
    client_udsock_path: &Path) -> io::Result<usize> {
        let msg_to_client = match result {
            Err(_) => MessageToClient::RequestError,
            Ok((thread_id, exit_status)) => {
                if let Some(monitor) = sunset_task(thread_id, running_tasks) {
                    // Deduct the completed tasks' filter counts from the server's.
                    filters_count.sub_assign(&monitor.task.get_transformations());

                    listener
                        .set_destination(client_udsock_path
                        .join(String::from("sdstore_") + &monitor.task.client_pid.to_string() + &".sock"))?;
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

        // TODO: this unwrap can be handled, at the expense of quite a bit of additional code
        let bytes = bincode::serialize(&msg_to_client).unwrap();
        listener.send(&bytes)
}

fn task_steal(
    task_pqueue: &mut PriorityQueue<ClientTask, usize>,
    filters_count: &mut RunningFilters,
    running_tasks: &mut HashMap<ThreadId, Monitor>,
    server_config: &ServerConfig,
    task_counter: &mut usize,
    sender: Sender<messaging::MessageToServer>,
    listener: &Arc<UdSocket>,
    client_udsock_path: &Path
) {
    while let Some((task, _)) = task_pqueue.peek() {
        if filters_count.can_run_pipeline(&server_config.filters_config, &task.transformations) {
            // Since the loop is only entered if the queue's highest priority element can be
            //peeked into, this unwrap is safe.
            let (task, _) = task_pqueue.pop().unwrap();

            // TODO: Handle this unwrap
            listener
                .set_destination(client_udsock_path
                .join(String::from("sdstore_") + &task.client_pid.to_string() + &".sock")).unwrap();
            let msg_to_client = MessageToClient::Processing;
        
            let bytes = bincode::serialize(&msg_to_client).unwrap();
            // TODO: handle this unwrap
            listener.send(&bytes).unwrap();

            // update server's limits with new task's counts.
            filters_count.add_assign(&task.transformations);

            let sender_clone = sender.clone();
            // TODO: don't unwrap here
            let monitor = Monitor::build(
                task,
                *task_counter,
                server_config.transformations_path(),
                sender_clone).unwrap();
            running_tasks.insert(monitor.thread_id(), monitor);

            // update server's task counter
            *task_counter += 1;
        }
    }
}

fn main() {
    // Init logging
    rust_sdstore::util::init_logging_infrastructure(
        None,
        log::LevelFilter::Trace
    ).unwrap_or_else(|err| {
        eprintln!("Could not init logging infrastructure! Error: {:?}", err);
        eprintln!("Exiting");
        std::process::exit(1);
    });

    // Read the server's configs from args: file with max filter definitions, and binary folder path
    let config = server_config::ServerConfig::build(&mut env::args())
        .unwrap_or_else(|err| {
            log::error!("Problem parsing config: {:?}", err);
            process::exit(1);
        });
    log::info!("Read config:\n{:?}", config);

    let curr_dir = std::env::current_dir().unwrap_or_else(|err| {
        log::error!("Could not get pwd. Error {:?}", err);
        process::exit(1);
    });
    // Init socket file
    let udsock_dir = curr_dir.join("tmp");
    log::info!("dir to be used for udsock is {:?}", udsock_dir);

    // Init the Unix domain socket
    let server_udsock = udsock_dir.join("sdstored.sock");
    match fs::remove_file(server_udsock.clone()) {
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => {},
        Err(err) => {
            log::error!("could not unlink existing server udsocket. Error: {:?}", err);
            process::exit(1);
        },
        Ok(_) => {}
    };
    let listener =
        udsocket::UdSocket::bind_with_drop_guard(server_udsock.as_path())
            .unwrap_or_else(|err| {
                log::error!("Could not create listener on socket. Error: {:?}", err);
                process::exit(1);
            });
    log::info!("server listening on Unix datagram socket: {:?}", listener);
    let listener = Arc::new(listener);

    // Used to number each request.
    let mut counter = 0;
    let mut filters_count = limits::RunningFilters::default();
    let mut task_pqueue = PriorityQueue::<ClientTask, usize>::new();
    let mut running_tasks = HashMap::<ThreadId, Monitor>::new();
    let (sender, receiver) = mpsc::channel::<messaging::MessageToServer>();

    let listener_clone = Arc::clone(&listener);
    let sender_clone = sender.clone();
    let listening_thread = thread::Builder::new()
        .name(String::from("sdstored_udsock_listener"))
        .spawn(move || udsock_listen(listener_clone, sender_clone))
        .unwrap_or_else(|err| {
            log::error!("Could not spawn UdSocket listening thread. Error: {:?}", err);
            process::exit(1);
        });

    // Loop the processing clients' and monitors' messages.
    loop {
        let msg = match receiver.recv() {
            Err(err) => {
                log::warn!("could not read from message receiver. Error: {:?}", err);
                break;
            },
            Ok(t) => t
        };
        match msg {
            messaging::MessageToServer::Client(req) => {
                match req {
                    messaging::ClientRequest::Status => {},
                    messaging::ClientRequest::ProcFile(task) => {
                        // TODO: handle this unwrap
                        accept_task(
                            &mut task_pqueue,
                            &listener,
                            &udsock_dir,
                            task,
                            ).unwrap();
                    },
                }
            }
            messaging::MessageToServer::Monitor(res) => {
                match process_task_result(
                    res,
                    &mut running_tasks,
                    &mut filters_count,
                    &listener,
                    &udsock_dir) {
                    // TODO
                    Err(_) => {}
                    Ok(_)  => {}
                }
            }
        }

    let sender_clone = sender.clone();
    task_steal(
        &mut task_pqueue,
        &mut filters_count,
        &mut running_tasks,
        &config,
        &mut counter,
        sender_clone,
        &listener,
        &udsock_dir)

    }
}