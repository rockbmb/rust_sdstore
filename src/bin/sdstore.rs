use rust_sdstore;

use interprocess::os::unix::udsocket;

use std::{fs, io, process};

fn main() {
    rust_sdstore::util::init_logging_infrastructure(
        None, 
        log::LevelFilter::Trace
    ).unwrap_or_else(|err| {
        eprintln!("Could not init logging infrastructure! Error: {:?}", err);
        eprintln!("Exiting");
        std::process::exit(1);
    });

    let udsock_dir = std::env::current_dir().unwrap_or_else(|err| {
            log::error!("Could not get pwd. Error {:?}", err);
            process::exit(1);
        }).join("tmp");
    log::info!("dir to be used for udsock is {:?}", udsock_dir);

    let client_udsock = udsock_dir.join(format!("sdstore_{}.sock", process::id()));
    let listener = udsocket::UdSocket::bind_with_drop_guard(client_udsock.as_path()).unwrap_or_else(|err| {
        log::error!("sdstored: Could not create listener on socket. Error: {:?}", err);
        process::exit(1);
    });
    log::info!("client listening on Unix datagram socket: {:?}", listener);

    let server_udsock = udsock_dir.join("sdstored.sock");
    listener.set_destination(server_udsock.as_path()).unwrap_or_else(|err| {
        log::error!("sdstore: error setting client socket destination. Error: {:?}", err);
        std::process::exit(1);
    });

    let mut msg = String::new();
    loop {
        io::stdin()
            .read_line(&mut msg)
            .unwrap_or_else(|err| {
                log::error!("sdstore: could not read from STDIN. Error: {:?}", err);
                std::process::exit(1);
            });

        listener.send(msg.as_bytes()).unwrap_or_else(|err| {
            log::error!("sdstored: Could not send to UdSocket. Error: {:?}", err);
            process::exit(1);
        });
        log::info!("sdstore: wrote\n{} to UdSocket", msg);

        if msg.len() == 0 { break }
        msg.clear();
    }

    fs::remove_file(client_udsock).unwrap_or_else(|err| {
        log::error!("could not unlink client udsocket. Error: {:?}", err);
    });
    log::info!("Exiting!");
}