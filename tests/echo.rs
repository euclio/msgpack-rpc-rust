extern crate msgpack_rpc;
extern crate rmp as msgpack;
extern crate mioco;

use std::io;
use std::io::prelude::*;
use std::net::{SocketAddr, ToSocketAddrs, TcpListener};
use std::thread;

use mioco::tcp::TcpListener as NonblockingTcpListener;
use msgpack::Value;
use msgpack_rpc::*;

pub struct EchoServer {
    listener: TcpListener,
}

impl EchoServer {
    fn new<A>(addr: A) -> EchoServer
        where A: ToSocketAddrs
    {
        let socket = addr.to_socket_addrs().unwrap().next().unwrap();

        EchoServer { listener: TcpListener::bind(&socket).unwrap() }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    fn serve(self) {
        let listener = self.listener.try_clone().unwrap();
        let local_addr = self.local_addr().unwrap().clone();

        mioco::start(move || {
            let listener = NonblockingTcpListener::from_listener(listener, &local_addr).unwrap();
            loop {
                let mut conn = try!(listener.accept());

                loop {
                    let request = try!(msgpack_rpc::Message::unpack(&mut conn));
                    let mut conn = conn.try_clone().unwrap();

                    mioco::spawn(move || {

                        match request {
                            Message::Request(Request { id, method, params }) => {
                                let response = if method == "echo" {
                                    Message::Response(Response {
                                        id: id,
                                        result: Ok(Value::Array(params.to_owned())),
                                    })
                                } else {
                                    Message::Response(Response {
                                        id: id,
                                        result: Err(Value::String("Invalid method name."
                                                                      .to_owned())),
                                    })
                                };

                                conn.write_all(&response.pack())
                            }
                            _ => panic!(),
                        }
                    })
                }
            }
        });
    }
}

#[test]
fn echo() {
    let server = EchoServer::new("localhost:0");
    let mut client = Client::new(server.local_addr().unwrap());

    thread::spawn(move || {
        server.serve();
    });

    let result = client.call("echo", vec![Value::String("Hello, world!".to_owned())]);
    assert_eq!(Value::Array(vec![Value::String("Hello, world!".to_owned())]),
               result.unwrap());
}

#[test]
fn invalid_method_name() {
    let server = EchoServer::new("localhost:0");
    let mut client = Client::new(server.local_addr().unwrap());

    thread::spawn(move || {
        server.serve();
    });

    let result = client.call("bad_method", vec![]);
    assert_eq!(Value::String("Invalid method name.".to_owned()),
               result.unwrap_err());
}
