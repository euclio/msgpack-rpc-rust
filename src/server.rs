use std::io;
use std::io::prelude::*;
use std::net::{SocketAddr, TcpListener, ToSocketAddrs};

use mioco;
use mioco::tcp::TcpListener as NonblockingTcpListener;
use msgpack::Value;

use message::Message;
use message::Response;
use message::Request;

pub trait Dispatch {
    fn dispatch(&mut self, method: &str, args: Vec<Value>) -> Result<Value, Value>;
    fn notify(&mut self, method: &str, args: Vec<Value>) {}
}

pub struct Server {
    listener: TcpListener,
}

impl Server {
    pub fn bind<A>(addr: A) -> io::Result<Server>
        where A: ToSocketAddrs
    {
        TcpListener::bind(addr).map(|listener| Server { listener: listener })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    pub fn handle<D>(&self, mut dispatcher: D)
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
