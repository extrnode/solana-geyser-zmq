use core::time;
use std::thread;

fn main() {
    let ctx = zmq::Context::new();
    let socket = ctx.socket(zmq::PUSH).unwrap();

    let sndhwm = 1_000_000_000;
    socket.set_sndhwm(sndhwm).unwrap();
    socket.bind("tcp://*:2000").unwrap();

    let mut x = 0;
    loop {
        if socket
            .send(format!("{}", x).as_str(), zmq::DONTWAIT)
            .map_err(|_| println!("cannot send!"))
            .is_ok()
        {
            println!("{}", x);
        }
        thread::sleep(time::Duration::from_micros(100));
        x += 1;
    }
}
