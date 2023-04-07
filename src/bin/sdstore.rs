use rust_sdstore;

use interprocess::os::unix::udsocket;

use std::{io, process};

fn main() {
    rust_sdstore::util::init_logging_infrastructure(
        None, 
        log::LevelFilter::Trace
    ).unwrap_or_else(|err| {
        eprintln!("Could not init logging infrastructure! Error: {:?}", err);
        eprintln!("Exiting");
        std::process::exit(1);
    });

    let sock_path = format!("tmp/sdstore_{}.sock", process::id());

    let listener = udsocket::UdSocket::bind(sock_path).unwrap_or_else(|err| {
        log::error!("sdstored: Could not create listener on socket. Error: {:?}", err);
        process::exit(1);
    });
    log::info!("client listening on Unix datagram socket: {:?}", listener);
    listener.set_destination("./tmp/sdstored.sock").unwrap_or_else(|err| {
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
        log::info!("sdstore: wrote {} to UdSocket", msg);

        msg.clear();
    }
}