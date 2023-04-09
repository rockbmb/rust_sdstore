use rust_sdstore::*;

use interprocess::os::unix::udsocket;

use std::{env, process};

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
    let listener = udsocket::UdSocket::bind_with_drop_guard(client_udsock.as_path()).unwrap_or_else(|err| {
        log::error!("sdstored: Could not create listener on socket. Error: {:?}", err);
        process::exit(1);
    });
    log::info!("client listening on Unix datagram socket: {:?}", listener);

    let server_udsock = udsock_dir.join("sdstored.sock");
    listener.set_destination(server_udsock.as_path()).unwrap_or_else(|err| {
        log::error!("sdstore: error setting client socket destination. Error: {:?}", err);
        process::exit(1);
    });

    let request =
        client::ClientRequest::build(env::args(), client_pid)
            .unwrap_or_else(|err| {
                log::error!("Could not parse request from arguments. Error: {:?}", err);
                process::exit(1);
            });

    let msg = bincode::serialize(&request)
        .unwrap_or_else(|err| {
            log::error!("Could not serialize request. Error: {:?}", err);
            process::exit(1);
        });
    listener.send(msg.as_slice()).unwrap_or_else(|err| {
        log::error!("sdstored: Could not send to UdSocket. Error: {:?}", err);
        process::exit(1);
    });
    log::info!("sdstore: wrote\n{:?} to UdSocket", request);

    log::info!("Exiting!");
}