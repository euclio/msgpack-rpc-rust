use std::io;
use std::io::prelude::*;
use std::net::{SocketAddr, TcpListener, ToSocketAddrs};

use mioco;
use mioco::tcp::TcpListener as NonblockingTcpListener;
use msgpack::Value;

use message::Message;
use message::Response;
use message::Request;

/// A target of RPC requests.
///
/// When a msgpack-RPC request is sent to the server, the server will delegate to the implementor
/// of this trait.
pub trait Dispatch {

    /// Respond to a remote procedure call.
    ///
    /// In most implementations, the implementor should switch on the value of the method, and pass
    /// arguments on to other methods of the implementor.
    fn dispatch(&mut self, method: &str, args: Vec<Value>) -> Result<Value, Value>;

    /// Response to a remote notification.
    ///
    /// Notifications are similar to remote procedure calls, but they do not respond. The default
    /// implementation of this method does nothing.
    #[allow(unused_variables)]
    fn notify(&mut self, method: &str, args: Vec<Value>) {}
}

/// A msgpack-RPC server.
///
/// The server will response to RPC requests and notifications and dispatch them appropriately.
pub struct Server {
    listener: TcpListener,
}

impl Server {
    /// Bind to an address.
    ///
    /// The server will listen on this address for msgpack-RPC requests.
    pub fn bind<A>(addr: A) -> io::Result<Server>
        where A: ToSocketAddrs
    {
        TcpListener::bind(addr).map(|listener| Server { listener: listener })
    }

    /// Returns the address that this server is listening on.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Begin handling RPC requests and notifications, dispatching them to an implementor of
    /// `Dispatch`.
    ///
    /// This method does not return.
    pub fn handle<D>(&self, dispatcher: D)
        where D: Dispatch + Send + Sync + Clone + 'static
    {
        let listener = self.listener.try_clone().unwrap();
        let local_addr = self.local_addr().unwrap().clone();

        mioco::start(move || {
            let listener = NonblockingTcpListener::from_listener(listener, &local_addr).unwrap();
            loop {
                let mut conn = try!(listener.accept());

                loop {
                    let request = try!(Message::unpack(&mut conn));
                    let mut conn = conn.try_clone().unwrap();

                    let mut dispatcher = dispatcher.clone();
                    mioco::spawn(move || {
                        match request {
                            Message::Request(Request { id, method, params }) => {
                                let result = dispatcher.dispatch(&method, params);
                                let response = Message::Response(Response {
                                    id: id,
                                    result: result,
                                });

                                conn.write_all(&response.pack()).unwrap();
                            }
                            _ => unimplemented!(),
                        }

                        Ok(())
                    });
                }
            }
        });
    }
}
