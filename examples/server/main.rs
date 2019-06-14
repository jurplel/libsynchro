#![feature(async_await)]

use libsynchro::*;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::net::SocketAddr;

use futures::sync::mpsc;

use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

type ClientListArc = Arc<Mutex<HashMap<SocketAddr, mpsc::UnboundedSender<Command>>>>;

struct Client {
    synchro_conn: Arc<Mutex<SynchroConnection>>,
    addr: SocketAddr,
    client_list: ClientListArc,
}

impl Client {
    fn new(socket: TcpStream, client_list: ClientListArc) -> Self {
        let addr = socket.peer_addr().unwrap();
        println!("Client connected: {}", addr);

        let (sender, mut reciever) = mpsc::unbounded();
        client_list.lock().unwrap().insert(addr, sender);

        let callback_client_list = client_list.clone();
        let callback = move |cmd| {
            let mut clients = callback_client_list.lock().unwrap();
            for (peer_addr, sender) in clients.iter_mut() {
                if *peer_addr != addr {
                    sender.unbounded_send(cmd).unwrap();
                }
            }
        };

        let synchro_conn = Arc::new(Mutex::new(SynchroConnection::from_existing(socket, Box::new(callback))));

        let synchro_conn2 = synchro_conn.clone();

        tokio::spawn_async(async move { 
            while let Some(cmd) = reciever.next().await {
                let cmd = cmd.unwrap();
                synchro_conn2.lock().unwrap().send(cmd).unwrap();
            }
        });

        Client {
            synchro_conn,
            addr,
            client_list,
        }
    }

    fn spawn_jobs(&mut self) {
        let (send_job, receive_job) = self.synchro_conn.lock().unwrap().take_jobs();
        tokio::spawn_async(send_job);
        tokio::spawn_async(receive_job);
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        println!("Dropped client {}", self.addr);
        self.client_list.lock().unwrap().remove(&self.addr);
    }
}

fn main() {
    let listener = start_server(32019).unwrap();
    println!("Server started sucessfully on {}", listener.local_addr().unwrap());

    let client_list: ClientListArc = Arc::new(Mutex::new(HashMap::new()));

    tokio::run_async(async move {
        let mut incoming = listener.incoming();

        while let Some(stream) = incoming.next().await {
            let stream = stream.unwrap();

            let mut client = Client::new(stream, client_list.clone());

            client.spawn_jobs();
        }
    });
}

fn start_server(port: u16) -> Result<TcpListener, Box<dyn std::error::Error>> {
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?; // to-do: add ipv6
    let listener = TcpListener::bind(&addr)?;
    Ok(listener)
}