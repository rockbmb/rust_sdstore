use rust_sdstore::core::messaging::{self, MessageToClient};

use std::{env, process, os::unix::net::UnixDatagram, fs};

fn main() {
    rust_sdstore::util::init_logging_infrastructure(
        None, 
        log::LevelFilter::Trace
    ).unwrap_or_else(|err| {
        eprintln!("Could not init logging infrastructure! Error: {:?}", err);
        eprintln!("Exiting");
        process::exit(1);
    });

    let client_pid = process::id();

    let udsock_dir = std::env::current_dir().unwrap_or_else(|err| {
            log::error!("Could not get pwd. Error {:?}", err);
            process::exit(1);
        }).join("tmp");
    log::info!("dir to be used for udsock is {:?}", udsock_dir);

    let client_udsock = udsock_dir.join(format!("sdstore_{}.sock", client_pid));
    let listener = UnixDatagram::bind(client_udsock.as_path()).unwrap_or_else(|err| {
        log::error!("sdstored: Could not create listener on socket. Error: {:?}", err);
        process::exit(1);
    });
    log::info!("client listening on Unix datagram socket: {:?}", listener);

    let server_udsock = udsock_dir.join("sdstored.sock");

    let request =
        messaging::ClientRequest::build(env::args(), client_pid)
            .unwrap_or_else(|err| {
                log::error!("Could not parse request from arguments. Error: {:?}", err);
                process::exit(1);
            });

    let msg = bincode::serialize(&request)
        .unwrap_or_else(|err| {
            log::error!("Could not serialize request. Error: {:?}", err);
            process::exit(1);
        });
    listener.send_to(msg.as_slice(), server_udsock).unwrap_or_else(|err| {
        log::error!("sdstored: Could not send to UdSocket. Error: {:?}", err);
        process::exit(1);
    });
    log::info!("sdstore: wrote\n{:?} to UdSocket", request);

    let mut buf = [0; 64];
    // TODO:
    // This loop only breaks if the client receives an error from the socket, or
    // its request is concluded.
    //
    // Otherwise, it'll hang forever. This can be fixed with a timeout thread.
    loop {
        let n = listener.recv(&mut buf).unwrap_or_else(|err| {
            log::error!("Could not read from UdSocket. Error: {:?}", err);
            process::exit(1);
        });
        let msg: MessageToClient = match bincode::deserialize(&buf[..n]) {
            Err(err) => {
                log::warn!("Error deserializing message from socket: {:?}", err);
                log::warn!("Moving on to next message");
                break;
            },
            Ok(val) => val,
        };
        log::info!("{msg}");

        if msg == MessageToClient::Concluded || msg == MessageToClient::RequestError { break }
    }

    log::info!("Exiting!");
    drop(listener);
    // TODO If the client receives e.g. `SIGKILL` while waiting for a message, the socket file
    // will not be deleted.
    //
    // this can be fixed with the `signal_hook` crate, enabling us to install signal handlers.
    fs::remove_file(client_udsock).unwrap_or_else(|err| {
        log::error!("Error deleting client udsocket file: {:?}", err);
        process::exit(1);
    });
}