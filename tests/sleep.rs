extern crate msgpack_rpc;
extern crate rmp as msgpack;
extern crate mioco;

use std::io;
use std::io::prelude::*;
use std::net::{SocketAddr, ToSocketAddrs, TcpListener};
use std::thread;
use std::time::Duration;

use mioco::tcp::TcpListener as NonblockingTcpListener;
use msgpack::Value;
use msgpack::value::Integer;
use msgpack_rpc::*;

pub struct SleepServer {
    listener: TcpListener,
}

impl SleepServer {
    fn new<A>(addr: A) -> SleepServer
        where A: ToSocketAddrs
    {
        let socket = addr.to_socket_addrs().unwrap().next().unwrap();

        SleepServer { listener: TcpListener::bind(&socket).unwrap() }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    fn serve(self) {
        let local_addr = self.local_addr().unwrap().clone();

        mioco::start(move || {
            let listener = NonblockingTcpListener::from_listener(self.listener, &local_addr)
                               .unwrap();
            loop {
                let mut conn = try!(listener.accept());

                loop {
                    let request = try!(msgpack_rpc::Message::unpack(&mut conn));
                    let mut conn = conn.try_clone().unwrap();

                    mioco::spawn(move || {
                        match request {
                            Message::Request(Request { id, params, .. }) => {
                                if let Value::Integer(Integer::U64(secs)) = *params.get(0)
                                                                                   .unwrap() {
                                    thread::sleep(Duration::from_secs(secs));
                                } else {
                                    panic!("Invalid parameters: {:?}", params);
                                }

                                let result = params.get(0).unwrap().to_owned();
                                let response = Message::Response(Response {
                                    id: id,
                                    result: Ok(result),
                                });

                                conn.write_all(&response.pack()).unwrap();

                                Ok(())
                            }
                            _ => panic!(),
                        }
                    });

                }
            }
        });
    }
}

#[test]
/// Ensures that requests that finish before long running requests are returned.
fn async() {
    let server = SleepServer::new("localhost:0");
    let mut client = Client::new(server.local_addr().unwrap());

    thread::spawn(move || {
        server.serve();
    });

    // Sleep for a very long time.
    client.async_call("sleep", vec![Value::Integer(Integer::U64(10_000))]);

    let short_result = client.async_call("sleep", vec![Value::Integer(Integer::U64(1))]);

    assert!(match short_result.recv().unwrap() {
        Ok(Value::Integer(Integer::U64(1))) => true,
        _ => false,
    });
}

#[test]
fn sleep() {
    let server = SleepServer::new("localhost:0");
    let mut client = Client::new(server.local_addr().unwrap());

    thread::spawn(move || {
        server.serve();
    });

    let long_result = client.async_call("sleep", vec![Value::Integer(Integer::U64(2))]);
    let short_result = client.async_call("sleep", vec![Value::Integer(Integer::U64(1))]);

    assert!(match short_result.recv().unwrap() {
        Ok(Value::Integer(Integer::U64(1))) => true,
        _ => false,
    });

    assert!(match long_result.recv().unwrap() {
        Ok(Value::Integer(Integer::U64(2))) => true,
        _ => false,
    });
}
