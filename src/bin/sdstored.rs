use rust_sdstore;

use std::env;
use std::process;

use tempfile::Builder;

fn main() {
    rust_sdstore::util::init_logging_infrastructure(
        None, 
        log::LevelFilter::Trace
    ).unwrap_or_else(|err| {
        eprintln!("Could not init logging infrastructure! Error: {:?}", err);
        eprintln!("Exiting");
        std::process::exit(1);
    });

    let config = rust_sdstore::Config::build(env::args()).unwrap_or_else(|err| {
        log::error!("sdstored: Problem parsing arguments: {:?}", err);
        process::exit(1);
    });
    log::info!("Read config:\n{:?}", config);

    let tmp_file = Builder::new()
        .prefix("sdstore")
        .suffix(".sock")
        .rand_bytes(0)
        .tempfile()
        .unwrap_or_else(|err| {
            log::error!("sdstored: Problem creating temporary file for unix socket! Error: {:?}", err);
            process::exit(1);
        });

    log::info!("filename for unix socket is {:?}", tmp_file);
}