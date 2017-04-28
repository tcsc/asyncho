
use std::collections::{BTreeMap, VecDeque};
use std::io::{self, ErrorKind};
use std::net::SocketAddr;
use std::mem::replace;

use futures::{Future, Stream};
use futures::unsync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures::stream::SplitSink;

use lines::LineCodec;
use tokio_core::reactor::{Core, Handle};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_io::AsyncRead;
use tokio_io::codec::Framed;

/// A shorthand definition of the Sink type for sending frames to a remote
/// client.
type FrameSender = SplitSink<Framed<TcpStream, LineCodec>>;

/// The range of messages our message handling function can deal with.
enum Msg {
    NewConnection { conn: TcpStream, remote_addr: SocketAddr },
    ConnectionLost { conn_id: usize },
    NewFrame { conn_id: usize, frame: String },
    FrameTxComplete { conn_id: usize, new_tx: FrameSender },
}

/// Represents an individual connection to the service. A `Conn` may either be
/// idle or busy sending data. Due to the way the `tokio` message sync works,
/// the easiest way to distinguish these two states is the presence or absence
/// of a value in the `frame_tx` field.
///
/// If the `frame_tx` field is is None, then connection is busy, and data
/// should be added to the send queue for later transmission. If the field is
/// not None, then the connection is ready to send, and frames should be sent
/// immediately.
pub struct Conn {
    queue: VecDeque<String>,
    frame_tx: Option<FrameSender>,
}

impl Conn {
    /// Constructs a new `Conn` that wraps the supplied `FrameSender` with some
    /// metadata.
    pub fn new(socket: FrameSender) -> Conn {
        Conn {
            queue: VecDeque::new(),
            frame_tx: Some(socket),
        }
    }
}

/// Implements the echo service.
pub struct Service {
    conns:        BTreeMap<usize, Conn>,
    msg_tx:       UnboundedSender<Msg>,
    msg_rx:       Option<UnboundedReceiver<Msg>>,
    message_loop: Handle,
    conn_count:   usize,
}

impl Service {
    /// Constructs a new Service object with sane defaults.
    pub fn new(h: &Handle) -> Service {
        info!("Creating messaging channels");
        let (tx, rx) = unbounded();

        Service {
            conns: BTreeMap::new(),
            msg_tx: tx,
            msg_rx: Some(rx),
            message_loop: h.clone(),
            conn_count: 0,
        }
    }

    /// Starts a listener on the supplied socket address.
    pub fn start_listener(&self, addr: &SocketAddr) -> io::Result<()> {
        info!("Starting TCP listener for {}", addr);

        // Start a TCP listener on a given port
        let listener = TcpListener::bind(addr, &self.message_loop)?;

        // Turn the listener into a stream of incoming connections, and wrap
        // it in a future that will process each incoming connection by posting
        // a NewConnection message to the main event loop.
        let tx = self.msg_tx.clone();
        let accept_handler = listener.incoming()
            .for_each(move |(conn, remote_addr)| {
                let msg = Msg::NewConnection {
                    conn: conn,
                    remote_addr: remote_addr
                };
                send_msg(&tx, msg)
            })
            .map_err(erase);

        // Start the future running on the event loop
        self.message_loop.spawn(accept_handler);
        Ok(())
    }

    /// Runs the service, blocking the calling thread until it returns.
    pub fn run(&mut self, msg_loop: &mut Core) {
        let rx = replace(&mut self.msg_rx, None).unwrap();
        let event_handler = rx.for_each(|msg| self.handle_message(msg));
        if let Err(_) = msg_loop.run(event_handler) {
            error!("Event loop returned error!")
        }
    }

    /// Main message handler. Invoked by the main message loop each time the
    /// message queue receives new data.
    fn handle_message(&mut self, msg: Msg) -> Result<(),()> {
        match msg {
            Msg::NewConnection {conn, remote_addr} => {
                info!("New connection from {}", remote_addr);
                self.spawn_connection(conn).map_err(erase);
                Ok(())
            },

            Msg::ConnectionLost {conn_id} => {
                info!("Conn {}: Connection lost", conn_id);
                self.conns.remove(&conn_id);
                Ok(())
            },

            Msg::NewFrame {conn_id, frame} => {
                info!("Conn {}: New frame: {}", conn_id, frame);
                if let Some(ref mut conn) = self.conns.get_mut(&conn_id) {
                    if conn.frame_tx.is_none() {
                        info!("Conn {}: connection busy, queuing frame.", conn_id);

                        conn.queue.push_back(frame)
                    } else {
                        let tx = replace(&mut conn.frame_tx, None).unwrap();
                        send_frame(conn_id, frame, tx, &self.msg_tx,
                                   &self.message_loop);
                    }
                }
                Ok(())
            },

            Msg::FrameTxComplete {conn_id, new_tx} => {
                info!("Conn {}: Send Complete.", conn_id);
                if let Some(ref mut conn) = self.conns.get_mut(&conn_id) {
                    match conn.queue.pop_front() {
                        Some(frame) => {
                            info!("Conn {}: Draining queue. ", conn_id);
                            send_frame(conn_id, frame, new_tx, &self.msg_tx, &self.message_loop)
                        },
                        None => {
                            conn.frame_tx = Some(new_tx)
                        }
                    }
                }
                Ok(())
            }
        }
    }

    /// Spawns a new connection onto the message loop and adds a corresponding
    /// `Conn` object the the `Service`'s connection list
    fn spawn_connection(&mut self, conn: TcpStream) -> io::Result<()> {
        // Bind the TCP stream to a framing algorithm that will chop the
        // incoming bytes into a sequence of well-defined frames, extracting
        // the tx and rx channels into separate objects.
        let (frame_tx, frame_rx) = conn.framed(LineCodec::new()).split();

        // Make copies of the channel back to the main message handler,
        // otherwise we'll have all sorts of lifetime issues because we can't
        // prove to the borrow checker that the references we keep in the
        // futures will not live longer than `self`.
        let tx_loop = self.msg_tx.clone();
        let tx_conn_lost =  self.msg_tx.clone();

        // generate an id number for the connection.
        let conn_id = self.conn_count;
        self.conn_count += 1;

        // Define a future that will iterate over the incoming frames, and then
        // signal the main loop when the connection is dropped.
        let frame_handler = frame_rx
            .for_each(move |frame| {
                let msg = Msg::NewFrame {
                    conn_id: conn_id,
                    frame: frame
                };
                send_msg(&tx_loop, msg)
            })
            .and_then(move |_| {
                send_msg(&tx_conn_lost, Msg::ConnectionLost {conn_id: conn_id})
            })
            .map_err(erase);

        // start the future executing on the message loop
        info!("Spawning frame handler");
        self.message_loop.spawn(frame_handler);

        // record the new connection in the service connection list
        let conn = Conn::new(frame_tx);
        self.conns.insert(conn_id, conn);
        Ok(())
    }
}

/// Sends a `Msg` on the supplied message channel, mapping the result to be
/// compatible with the futures library
fn send_msg(tx: &UnboundedSender<Msg>, msg: Msg) -> io::Result<()> {
    tx.send(msg).map_err(|e| io::Error::new(ErrorKind::Other, e))
}

/// Sends a frame on the supplied `FrameSender`, consuming the sender and
/// sending a `FrameTxComplete` message to the main message loop when it's
/// done.
fn send_frame(conn_id: usize,
              frame: String,
              tx: FrameSender,
              channel_ref: &UnboundedSender<Msg>,
              message_loop: &Handle) {
    use futures::Sink;
    let channel = channel_ref.clone();

    let send_frame =
        tx.send(frame)
            .and_then(move |new_tx| {
                let msg = Msg::FrameTxComplete {
                    conn_id: conn_id,
                    new_tx: new_tx
                };
                send_msg(&channel, msg)
            })
            .map_err(erase);

    message_loop.spawn(send_frame)
}

fn erase<T>(_: T) -> () {
    ()
}