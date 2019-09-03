use libsynchro;

use std::net::SocketAddr;
use std::env;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let addr_str;
    if args.len() < 2 {
        addr_str = "127.0.0.1:32019";
    } else {
        addr_str = args[1].as_ref();
    }

    println!("server list: {:?}", libsynchro::get_server_list(None).unwrap());

    use libsynchro::SynchroConnection;
    let addr: SocketAddr = addr_str.parse().unwrap();
    let mut conn = SynchroConnection::new_async(addr, Box::new(|_| {})).await.unwrap();

    let (send_job, receive_job) = conn.take_jobs();
    tokio::spawn(async move {
        tokio::spawn(receive_job);
        send_job.await;
    });

    // Set name for test purposes
    let name = String::from("Rusty Shackleford");
    conn.send(libsynchro::Command::SetName {desired_name: name}).unwrap();

    std::thread::park();
}
