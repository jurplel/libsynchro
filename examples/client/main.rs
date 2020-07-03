use libsynchro;

use std::net::SocketAddr;
use std::env;

use async_std::task;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    task::block_on(connect())
}

async fn connect() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let addr_str;
    if args.len() < 2 {
        addr_str = "127.0.0.1:32019";
    } else {
        addr_str = args[1].as_ref();
    }

    println!("server list: {:?}", libsynchro::get_server_list(None)?);

    use libsynchro::SynchroConnection;
    let addr: SocketAddr = addr_str.parse()?;
    let mut conn = SynchroConnection::new_async(addr).await?;

    let handle = conn.run();

    // Set name for test purposes
    let name = String::from("Rusty Shackleford");
    conn.send(libsynchro::Command::SetName {desired_name: name})?;

    handle.await;

    Ok(())
}