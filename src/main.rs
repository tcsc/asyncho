extern crate bytes;
extern crate env_logger;
extern crate futures;
#[macro_use] extern crate log;
extern crate tokio_core;
extern crate tokio_io;

mod lines;
mod service;

use service::Service;
use tokio_core::reactor::Core;
fn main() {
    use std::process::exit;
    env_logger::init().unwrap();

    info!("Creating task executor");
    let mut core = Core::new().unwrap();
    let h = core.handle();

    info!("Creating service data structures");
    let mut service = Service::new(&h);

    let addr = "127.0.0.1:4444".parse().unwrap();
    match service.start_listener(&addr) {
        Ok(_) => info!("Listener started"),
        Err(e) => {
            error!("Listener failed to start: {}. Bailing!", e);
            exit(1);
        }
    }

    service.run(&mut core);
}
