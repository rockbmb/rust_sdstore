use rust_sdstore::{config, client::ClientRequest, monitor};

use interprocess::os::unix::udsocket;

use std::{env, process, fs, io};


fn main() {
    rust_sdstore::util::init_logging_infrastructure(
        None, 
        log::LevelFilter::Trace
    ).unwrap_or_else(|err| {
        eprintln!("Could not init logging infrastructure! Error: {:?}", err);
        eprintln!("Exiting");
        std::process::exit(1);
    });

    let config = config::ServerConfig::build(&mut env::args())
        .unwrap_or_else(|err| {
            log::error!("Problem parsing config: {:?}", err);
            process::exit(1);
        });
    log::info!("Read config:\n{:?}", config);

    let udsock_dir = std::env::current_dir().unwrap_or_else(|err| {
            log::error!("Could not get pwd. Error {:?}", err);
            process::exit(1);
        }).join("tmp");
    log::info!("dir to be used for udsock is {:?}", udsock_dir);

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

    let mut buf = [0; 1024];
    loop {
        let (n, _) = listener.recv(&mut buf).unwrap_or_else(|err| {
            log::error!("Could not read from UdSocket. Error: {:?}", err);
            process::exit(1);
        });
        if n <= 1 { break }
        let msg = &buf[..n].to_vec();
        let request: ClientRequest = bincode::deserialize(msg).unwrap_or_else(|err| {
            log::error!("Could not parse client request. Error: {:?}", err);
            process::exit(1);
        });

        match request {
            ClientRequest::Status => {},
            ClientRequest::ProcFile(task) => {
                log::info!("executing request \n{:?}\nfrom UdSocket", task);
                match monitor::start_pipeline_monitor(task, config.transformations_path()) {
                    Err(err) => log::error!("client request failed with {:?}", err),
                    Ok(_) => log::info!("client request succeeded."),
                };
            }
        }
    }

    log::info!("Exiting!");
}