use rust_sdstore::Config;

use std::env;
use std::process;

fn main() {
    let config = Config::build(env::args()).unwrap_or_else(|err| {
        eprintln!("sdstored: Problem parsing arguments: {:?}", err);
        process::exit(1);
    });

    println!("Read config:\n{:#?}", config);
}