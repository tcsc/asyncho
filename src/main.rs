extern crate env_logger;
extern crate futures;
#[macro_use] extern crate log;
extern crate tokio_core;

use std::io::{self, ErrorKind};
use std::net::SocketAddr;

use futures::{Future, Stream};
use futures::unsync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use tokio_core::reactor::{Core, Handle};
use tokio_core::net::{TcpListener, TcpStream};


enum Msg {
    NewConnection (TcpStream, SocketAddr)
}

fn main() {
    use std::process::exit;

    info!("Creating task executor");
    let mut core = Core::new().unwrap();
    let h = core.handle();

    info!("Creating messaging channels");
    let (tx, rx) = unbounded();

    let addr = "127.0.0.1:4444".parse().unwrap();
    info!("Starting TCP listrener for {}", addr);
    match start_listener(&addr, tx.clone(), &h) {
        Ok(_) => info!("Listener started"),
        Err(e) => {
            error!("Listener failed to start: {}. Bailing!", e);
            exit(1);
        } 
    }

    info!("Starting main event loop");
    let event_handler = rx.for_each(|msg| handle_event(&tx, msg));
    if let Err(_) = core.run(event_handler) {
        error!("Event loop returned error!")
    }
}

fn handle_event(tx: &UnboundedSender<Msg>, msg: Msg) -> std::result::Result<(), ()> {
    match msg {
        Msg::NewConnection (conn, addr) => {
            info!("New connection from {}", addr);
            Ok(())
        }
    }
}

fn start_listener(
        addr: &SocketAddr, 
        tx: UnboundedSender<Msg>,
        h: &Handle) 
            -> io::Result<()> {
    let listener = TcpListener::bind(addr, &h)?;
    let accept_handler = listener.incoming()
        .for_each(
            move |(conn, remote_addr)| 
                tx.send(Msg::NewConnection (conn, remote_addr))
                  .map_err(|e| io::Error::new(ErrorKind::Other, e))
        )
        .map_err(|_| ());

    h.spawn(accept_handler);
    Ok(())
}