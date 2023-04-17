use std::{
    env, process, fs, io, sync::{mpsc, Arc},
    os::unix::net::UnixDatagram
};


use rust_sdstore::{
    core::{
        messaging::ClientRequest,
        server::{config, state::ServerState},
        messaging::MessageToServer
    }
};

fn udsock_listen(listener: Arc<UnixDatagram>, sender: mpsc::Sender<MessageToServer>) {
    // Loop the processing of clients' requests.
    let mut buf = [0; 1024];
    loop {
        let n = listener.recv(&mut buf).unwrap_or_else(|err| {
            log::error!("Could not read from UdSocket. Error: {:?}", err);
            process::exit(1);
        });
        // TODO: handle this unwrap
        let request: ClientRequest = bincode::deserialize(&buf[..n]).unwrap();

        // TODO: this unwrap needs to be handled
        sender.send(MessageToServer::Client(request)).unwrap();
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
    let config = config::ServerConfig::build(&mut env::args())
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
        UnixDatagram::bind(server_udsock.as_path())
            .unwrap_or_else(|err| {
                log::error!("Could not create listener on socket. Error: {:?}", err);
                process::exit(1);
            });
    log::info!("server listening on Unix datagram socket: {:?}", listener);

    let mut server_state = ServerState::new(listener, udsock_dir);

    let sender_clone = server_state.get_sender().clone();
    let listener_clone = server_state.get_udsocket();
    server_state.spawn_udsock_mngr(
        "sdstored_udsock_listener",
        Box::new(move || udsock_listen(listener_clone, sender_clone))
    ).unwrap_or_else(|err| {
        log::error!("Could not spawn UdSocket listening thread. Error: {:?}", err);
        process::exit(1);
    });

    // Loop the processing clients' and monitors' messages.
    loop {
        let msg = match server_state.receiver.recv() {
            Err(err) => {
                log::warn!("could not read from message receiver. Error: {:?}", err);
                break;
            },
            Ok(t) => t
        };
        match msg {
            MessageToServer::Client(ClientRequest::Status) => {
                // TODO: return server status to client
            }
            MessageToServer::Client(ClientRequest::ProcFile(task)) => {
                log::info!("Queueing received task:\n{:?}", task);
                // TODO: handle this unwrap
                server_state.receive_task(task).unwrap();
            }
            MessageToServer::Monitor(res) => {
                match server_state.handle_task_result(res) {
                    // TODO
                    Err(_) => {}
                    Ok(_)  => {}
                }
            }
        }

    while let Some(task) = server_state.try_pop_task(&config) {
        let client_pid = task.client_pid;
        log::info!("Executing task popped from pqueue:\n{:?}", task);
        let (mon_id, task_num) = server_state.process_task(&config, task);
        log::info!("Task by client {client_pid} assigned number {task_num} and monitor {:?}", mon_id);
    }

    }
}