extern crate rmp as msgpack;
extern crate msgpack_rpc;

use std::thread;

use msgpack::Value;
use msgpack_rpc::*;

#[derive(Clone)]
struct EchoCallClient;

#[derive(Clone)]
struct EchoCallServer;

impl Dispatch for EchoCallServer {
    fn dispatch(&mut self, method: &str, args: Vec<Value>) -> Result<Value, Value> {
        match method {
            "call" => Ok(Value::Array(args.to_owned())),
            _ => Err(Value::String("Invalid server method name.".to_owned())),
        }
    }
}

impl Dispatch for EchoCallClient {
    fn dispatch(&mut self, method: &str, args: Vec<Value>) -> Result<Value, Value> {
        match method {
            "call" => {
                match *args.get(1).unwrap() {
                    Value::String(ref method) if method == "echo" => {
                        Ok(Value::Array(args.to_owned().split_off(2)))
                    }
                    _ => Err(Value::String("Attempted to call invalid method.".to_owned())),
                }
            }
            _ => Err(Value::String("Invalid client method name.".to_owned())),
        }
    }
}

#[test]
#[should_panic]
fn default_client_dispatch() {
    let server = Server::bind("localhost:0").unwrap();
    let mut client = Client::new().connect_socket(server.local_addr().unwrap());

    thread::spawn(move || {
        server.handle(EchoCallServer);
    });

    let result = client.call("call",
                             vec![Value::String("echo".to_owned()),
                                  Value::String("Hello, world!".to_owned())]);
    result.unwrap();
}

#[test]
fn bidirectional_client() {
    let server = Server::bind("localhost:0").unwrap();
    let mut client = Client::with_dispatcher(EchoCallClient)
                         .connect_socket(server.local_addr().unwrap());

    thread::spawn(move || {
        server.handle(EchoCallServer);
    });

    let result = client.call("call",
                             vec![Value::String("echo".to_owned()),
                                  Value::String("Hello, world!".to_owned())]);
    result.unwrap();
}
