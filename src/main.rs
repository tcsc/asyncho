extern crate core;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate twist;


#[macro_use]
extern crate slog;
extern crate slog_term;

mod websockets;
mod messaging;

use slog::*;
use std::fmt;
use std::thread::{self, JoinHandle};
use std::str::FromStr;
use std::time::Duration;
use tokio_core::reactor::{Core, Handle, Interval};
use twist::server::TwistCodec;

use messaging::MessageRouter;
use websockets::WebsocketServer;

fn main() {
    use std::process::exit;

    let console_drain = slog_term::streamer().build();
    let logger = slog::Logger::root(console_drain.fuse(), o!());

    let mut core = Core::new().unwrap();
    let h = core.handle();

    let addr = "127.0.0.1:4444".parse().unwrap();
    let server = WebsocketServer::new(addr, &h);

    println!("{:?} Starting timer", thread::current());
    core.run(futures::future::empty::<(),()>()).unwrap();

    println!("Hello, world!");
}