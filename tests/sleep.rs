extern crate msgpack_rpc;
extern crate rmp as msgpack;

use std::thread;
use std::time::Duration;

use msgpack::Value;
use msgpack::value::Integer;
use msgpack_rpc::*;

#[derive(Clone)]
pub struct SleepServer;

impl Dispatch for SleepServer {
    fn dispatch(&mut self, method: &str, args: Vec<Value>) -> Result<Value, Value> {
        match method {
            "sleep" => {
                if let Value::Integer(Integer::U64(secs)) = *args.get(0)
                                                                 .unwrap() {
                    thread::sleep(Duration::from_secs(secs));
                    Ok(Value::Integer(Integer::U64(secs)))
                } else {
                    Err(Value::String(format!("Invalid parameters: {:?}", args)))
                }
            }
            _ => Err(Value::String(format!("Invalid method: {}", method))),
        }
    }
}

#[test]
/// Ensures that requests that finish before long running requests are returned.
fn async() {
    let server = Server::bind("localhost:0").unwrap();
    let mut client = Client::connect(server.local_addr().unwrap());

    thread::spawn(move || server.handle(SleepServer));

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
    let server = Server::bind("localhost:0").unwrap();
    let mut client = Client::connect(server.local_addr().unwrap());

    thread::spawn(move || server.handle(SleepServer));

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
