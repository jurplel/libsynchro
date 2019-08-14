use libsynchro;

use std::net::SocketAddr;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let addr_str;
    if args.len() < 2 {
        addr_str = "127.0.0.1:32019";
    } else {
        addr_str = args[1].as_ref();
    }

    use libsynchro::SynchroConnection;
    let addr: SocketAddr = addr_str.parse().unwrap();
    let mut conn = SynchroConnection::new(addr, Box::new(|_| {})).unwrap();
    conn.run();
}
