use rust_sdstore::*;

use interprocess::os::unix::udsocket;

use std::{env, process, str, fs};


fn main() {
    rust_sdstore::util::init_logging_infrastructure(
        None, 
        log::LevelFilter::Trace
    ).unwrap_or_else(|err| {
        eprintln!("Could not init logging infrastructure! Error: {:?}", err);
        eprintln!("Exiting");
        std::process::exit(1);
    });

    let config = config::Config::build(env::args()).unwrap_or_else(|err| {
        log::error!("sdstored: Problem parsing arguments: {:?}", err);
        process::exit(1);
    });
    log::info!("Read config:\n{:?}", config);

    let sock_path = create_file_for_udsock("tmp/sdstored.sock").unwrap_or_else(|err| {
        log::error!("sdstored: Could not create file for UdSocket. Error {:?}", err);
        process::exit(1);
    });
    log::info!("server's socket path is: {:?}", sock_path);

    fs::remove_file(sock_path.clone()).unwrap();
    let listener = udsocket::UdSocket::bind(sock_path).unwrap_or_else(|err| {
        log::error!("sdstored: Could not create listener on socket. Error: {:?}", err);
        process::exit(1);
    });
    log::info!("server listening on Unix datagram socket: {:?}", listener);

    let mut buf = [0; 1024];

    loop {
        let (n, _) = listener.recv(&mut buf).unwrap_or_else(|err| {
            log::error!("sdstored: Could not read from UdSocket. Error: {:?}", err);
            process::exit(1);
        });
        let msg = &buf[..n];

        log::info!("sdstored: read {}\n from UdSocket", str::from_utf8(msg).unwrap());
    }
}