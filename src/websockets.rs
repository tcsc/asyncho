use futures::{Future, Stream};
use futures::unsync::mpsc::unbounded;
use tokio_core::reactor::Handle;
use tokio_core::net::TcpListener;
use std::fmt;
use std::net::SocketAddr;
use std::thread;

pub enum WsError {
    AddrInUse,
}

impl fmt::Display for WsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match self {
            AddrInUse => "Address In Use",
        };
        f.write_str(msg)
    }
}

pub type WsResult<T> = Result<T, WsError>;

pub struct WebsocketServer {
}

impl WebsocketServer {
    pub fn new(addr: SocketAddr, h: &Handle) -> WsResult<WebsocketServer> {
        use core::cell::RefCell;

        let listener = try!(TcpListener::bind(&addr, h)
                            .map_err(|_| WsError::AddrInUse));

        // create a channel that this listener can report new connections on
        let (mut connection_tx, connection_rx) = unbounded();

        // Set up a task that will run every time a new connection is received
        let pickup = connection_rx.for_each(|(s, addr)| {
            println!("{:?} New connection from {}", thread::current(), addr);
            Ok(())
        });
        h.spawn(pickup);

        // Convert the TCP listener into a future that will produce a stream of
        // incoming connections, each of which will then be routed back along the
        // channel we just created
        let handler = listener.incoming().for_each(move |c| {
            println!("{:?} Connection accepted", thread::current());
            if let Err(e) = connection_tx.send(c) {
                println!("{:?} sending inbound connection failed: {}", thread::current(), e)
            };
            Ok(())
        });

        h.spawn(handler.map_err(|_| ()));

        Ok(WebsocketServer{})
    }
}


//struct HandleConnection {
//    tx: Sender<(TcpStream, SocketAddr)>
//}
//
//impl FnMut<(TcpStream, SocketAddr)> for HandleConnection {
//    fn call_mut(&mut self, conn: (TcpStream, SocketAddr)) -> Result<(), io::Error> {
//        if let Ok(tx) = self.tx.send(conn).wait() {
//            self.tx = tx;
//        }
//        Ok(())
//    }
//}

//fn handle_connection(s: TcpStream) -> io::Result<()> {
//    let transport = s.framed(TwistCodec::default());
//    transport.for_each
//}