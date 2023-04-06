use rust_sdstore;

use std::env;
use std::process;

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
        log::warn!("sdstored: Problem parsing arguments: {:?}", err);
        process::exit(1);
    });

    log::info!("Read config:\n{:#?}", config);
}