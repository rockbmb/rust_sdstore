use std::{
    env, process, fs, io, os::unix::net::UnixDatagram
};


use rust_sdstore::{
    core::{
        messaging::ClientRequest,
        server::{config, state::ServerState},
        messaging::MessageToServer
    }
};

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
    let server_config = config::ServerConfig::build(&mut env::args())
        .unwrap_or_else(|err| {
            log::error!("Problem parsing config: {:?}", err);
            process::exit(1);
        });
    log::info!("Read config:\n{:?}", server_config);

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

    server_state
        .spawn_udsock_mngr("sdstored_udsock_listener")
        .unwrap_or_else(|err| {
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
                let client_pid = task.client_pid;
                log::info!("Attempting to queueing received task:\n{:?}", task);
                match server_state.new_task(task) {
                    Ok(_) => log::info!("Successfully queued task by client PID {client_pid}"),
                    Err(err) => log::error!("Failed to queue task by client PID {client_pid}: {:?}", err),
                }
            }
            MessageToServer::Monitor(res) => {
                let t_id = res.thread;
                let cl_pid = match server_state.client_pid_from_monitor_id(&t_id) {
                    None => {
                        log::error!("message received from nonexistent monitor!");
                        break;
                    }
                    Some(t) => t
                };
                match server_state.handle_task_result(res) {
                    Err(err) => log::error!("Monitor {:?} for task by client {cl_pid} failed: {:?}", t_id, err),
                    Ok(_)  => log::info!("Monitor {:?} for task by client {cl_pid} succeeded.", t_id)
                }
            }
        }

        while let Some(task) = server_state.try_pop_task(&server_config) {
            let client_pid = task.client_pid;
            log::info!("Executing task popped from pqueue:\n{:?}", task);
            match server_state.process_task(&server_config, task) {
                Err(err) => log::error!("Failed to process task by client PID {client_pid}: {:?}", err),
                Ok((mon_id, task_num)) =>
                    log::info!("Task by client {client_pid} assigned number {task_num} and monitor {:?}", mon_id)
            }
        }

    }
}