use libsynchro;

use std::net::SocketAddr;

fn main() {
    use libsynchro::SynchroConnection;
    let addr: SocketAddr = format!("127.0.0.1:{}", 32019).parse().unwrap();
    let mut conn = SynchroConnection::new(addr, Box::new(|_|{})).unwrap();
    conn.run();
}