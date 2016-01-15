use std::collections::HashMap;
use std::i32;
use std::io::prelude::*;
use std::net::{TcpStream, ToSocketAddrs};
use std::thread;
use std::sync::{mpsc, Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

use msgpack::Value;

use ::message::*;

type MessageId = i32;

/// A new msgpack-RPC client.
///
/// The client connects to a server through a transport, and sends and receives RPC messages
/// through that transport. Messages may be sent synchronously or asynchronously. Similarly,
/// responses may be received synchronously or asynchronously.
pub struct Client {
    sender: mpsc::Sender<Message>,
    request_map: Arc<Mutex<HashMap<MessageId, mpsc::Sender<Result<Value, Value>>>>>,
    id_generator: AtomicUsize,
}

impl Client {
    /// Connect to a msgpack-RPC server.
    ///
    /// This function returns a client that is bound to a particular socket address.
    pub fn connect<A>(transport: A) -> Client
        where A: ToSocketAddrs
    {
        let transport = TcpStream::connect(transport).unwrap();
        let (sender, receiver) = mpsc::channel();

        let request_map = Arc::new(Mutex::new(HashMap::new()));

        let local_request_map = request_map.clone();

        // Requests
        let mut writer = transport.try_clone().unwrap();
        thread::Builder::new()
            .name("client request handler".to_owned())
            .spawn(move || {
                for request in receiver.iter() {
                    let request = match request {
                        Message::Request(request) => request,
                        _ => unimplemented!(),
                    };

                    writer.write_all(&Message::Request(request).pack()).unwrap();
                }
            })
            .unwrap();

        // Responses
        let mut reader = transport.try_clone().unwrap();
        thread::Builder::new()
            .name("client response handler".to_owned())
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
                        _ => unimplemented!(),
                    }
                }
            })
            .unwrap();

        Client {
            sender: sender,
            request_map: request_map,
            id_generator: AtomicUsize::new(0),
        }
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
        self.sender.send(Message::Request(request)).unwrap();

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
