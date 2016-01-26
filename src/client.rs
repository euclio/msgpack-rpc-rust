use std::collections::HashMap;
use std::i32;
use std::io::prelude::*;
use std::net::{TcpStream, ToSocketAddrs};
use std::thread;
use std::sync::{mpsc, Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

use mioco::Evented;
use mioco::unix;
use msgpack::Value;

use ::message::*;
use server::Dispatch;

type MessageId = i32;

/// A new msgpack-RPC client.
///
/// The client connects to a server through a transport, and sends and receives RPC messages
/// through that transport. Messages may be sent synchronously or asynchronously. Similarly,
/// responses may be received synchronously or asynchronously.
pub struct Client;

/// Bleh
pub struct ClientHandle<D>
    where D: Dispatch + 'static
{
    dispatcher: D,
    sender: Option<mpsc::Sender<Message>>,
    request_map: Arc<Mutex<HashMap<MessageId, mpsc::Sender<Result<Value, Value>>>>>,
    id_generator: AtomicUsize,
}

impl Client {
    /// Creates a new Client.
    ///
    /// By default, the client panics when receiving notifications or requests. If you'd like to
    /// modify this behavior, implement `Dispatch` for a struct as pass it to `with_dispatch`.
    pub fn new() -> ClientHandle<ClientDispatch> {
        ClientHandle {
            id_generator: AtomicUsize::new(0),
            dispatcher: ClientDispatch,
            request_map: Arc::new(Mutex::new(HashMap::new())),
            sender: None,
        }
    }

    /// Bleh
    pub fn with_dispatcher<D>(dispatcher: D) -> ClientHandle<D>
        where D: Dispatch
    {
        ClientHandle {
            id_generator: AtomicUsize::new(0),
            dispatcher: dispatcher,
            request_map: Arc::new(Mutex::new(HashMap::new())),
            sender: None,
        }
    }
}

impl<D> ClientHandle<D> where D: Dispatch
{
    /// Connect to a msgpack-RPC server.
    ///
    /// This function returns a client that is bound to a particular socket address.
    pub fn connect_socket<A>(mut self, transport: A) -> Self
        where A: ToSocketAddrs
    {
        let transport = TcpStream::connect(transport).unwrap();
        let (sender, receiver) = mpsc::channel();
        self.sender = Some(sender);
        self.start_event_loop(transport.try_clone().unwrap(),
                              transport.try_clone().unwrap(),
                              receiver);
        self
    }

    /// Connect to a msgpack-RPC server through a pipe.
    pub fn connect_pipe(mut self) -> Self {
        let (stdin, stdout) = unix::pipe().unwrap();

        let (sender, receiver) = mpsc::channel();
        self.sender = Some(sender);
        self.start_event_loop(stdin, stdout, receiver);
        self
    }

    fn start_event_loop<R, W>(&self,
                              mut reader: R,
                              mut writer: W,
                              request_recv: mpsc::Receiver<Message>)
        where R: Read + Send + 'static,
              W: Write + Send + 'static
    {
        let (writer_sender, writer_receiver) = mpsc::channel();

        let request_writer_sender = writer_sender.clone();
        thread::Builder::new()
            .name("request_handler".to_owned())
            .spawn(move || {
                for request in request_recv.iter() {
                    if let Message::Request(..) = request {
                        request_writer_sender.send(request).unwrap();
                    } else {
                        unimplemented!();
                    }
                }
            })
            .unwrap();

        thread::Builder::new()
            .name("writer handler".to_owned())
            .spawn(move || {
                for message in writer_receiver.iter() {
                    writer.write_all(&message.pack()).unwrap();
                }
            })
            .unwrap();

        let local_request_map = self.request_map.clone();
        let mut dispatcher = self.dispatcher.clone();
        thread::Builder::new()
            .name("reader handler".to_owned())
            .spawn(move || {
                loop {
                    let message = Message::unpack(&mut reader).unwrap();

                    match message {
                        Message::Response(Response { id, result }) => {
                            let sender: mpsc::Sender<_> = local_request_map.lock()
                                                                           .unwrap()
                                                                           .remove(&id)
                                                                           .unwrap();
                            sender.send(result).unwrap();
                        }
                        Message::Request(Request { id, method, params }) => {
                            println!("ATTEMPTING DISPATCH");
                            let result = dispatcher.dispatch(&method, params);
                            println!("SUCCEEDED DISPATCH");
                            let response = Message::Response(Response {
                                id: id,
                                result: result,
                            });
                            writer_sender.send(response).unwrap();
                        }
                        Message::Notification(Notification { .. }) => unimplemented!(),
                    }
                }
            })
            .unwrap();

    }

    fn next_id(&mut self) -> MessageId {
        let ordering = Ordering::Relaxed;

        let id: MessageId = self.id_generator.fetch_add(1, ordering) as MessageId;

        if id == i32::MAX as MessageId {
            self.id_generator.store(0, ordering);
        }

        id
    }

    /// Sends a msgpack-RPC message asynchrouously.
    ///
    /// Returns a `mpsc::Channel` that can be blocked on for the result.
    pub fn async_call(&mut self,
                      method: &str,
                      params: Vec<Value>)
                      -> mpsc::Receiver<Result<Value, Value>> {
        let (tx, rx) = mpsc::channel();

        let request = Request {
            id: self.next_id(),
            method: method.to_owned(),
            params: params.to_owned(),
        };

        self.request_map.lock().unwrap().insert(request.id, tx);
        match self.sender {
            Some(ref sender) => sender.send(Message::Request(request)).unwrap(),
            _ => panic!("Client does not have a sender."),
        }

        rx
    }

    /// Sends a msgpack-RPC message synchrouously.
    ///
    /// The request will be sent, and the client will block until a response is received, returning
    /// the result.
    pub fn call(&mut self, method: &str, params: Vec<Value>) -> Result<Value, Value> {
        let receiver = self.async_call(method, params);
        receiver.recv().unwrap()
    }
}

/// The default implementation for client dispatch.
///
/// # Panics
/// This implementation panics on receiving a request or notification.
#[derive(Clone)]
pub struct ClientDispatch;

impl Dispatch for ClientDispatch {
    fn dispatch(&mut self, method: &str, args: Vec<Value>) -> Result<Value, Value> {
        panic!("Client received request.");
    }
}
