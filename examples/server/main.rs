#![feature(async_await)]

use libsynchro::*;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::net::SocketAddr;

use futures::sync::mpsc::{self, UnboundedSender};

use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

type ClientHashmapArc = Arc<Mutex<HashMap<SocketAddr, UnboundedSender<Command>>>>;

struct Client {
    synchro_conn: Arc<Mutex<SynchroConnection>>,
    addr: SocketAddr,
    client_hashmap: ClientHashmapArc,
}

impl Client {
    fn new(socket: TcpStream, client_hashmap: ClientHashmapArc) -> Self {
        let addr = socket.peer_addr().unwrap();
        println!("Client connected: {}", addr);

        let (sender, mut reciever) = mpsc::unbounded();
        client_hashmap.lock().unwrap().insert(addr, sender);

        let callback_client_hashmap = client_hashmap.clone();
        let callback = move |cmd: Command| {
            let clients = callback_client_hashmap.lock().unwrap();
            
            // Determine which clients to forward command to
            match cmd {
                Command::UpdateClientList {client_list: _} => {

                    let new_cmd = Command::UpdateClientList {client_list: get_list_of_clients(&clients)};

                    clients.get(&addr).unwrap().unbounded_send(new_cmd.clone()).unwrap();

                },
                _ => {
                    for (peer_addr, sender) in clients.iter() {
                        if *peer_addr != addr {
                            sender.unbounded_send(cmd.clone()).unwrap();
                        }
                    }
                },
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
            client_hashmap,
        }
    }

    fn take_jobs(&mut self) -> (AsyncJob, AsyncJob) {
        self.synchro_conn.lock().unwrap().take_jobs()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        println!("Dropped client {}", self.addr);
        let mut clients = self.client_hashmap.lock().unwrap();
        clients.remove(&self.addr);
        send_client_list_to_all(&clients);
    }
}

fn main() {
    let listener = start_server(32019).unwrap();
    println!("Server started sucessfully on {}", listener.local_addr().unwrap());

    let client_hashmap: ClientHashmapArc = Arc::new(Mutex::new(HashMap::new()));

    tokio::run_async(async move {
        let mut incoming = listener.incoming();

        while let Some(stream) = incoming.next().await {
            let stream = stream.unwrap();

            let mut client = Client::new(stream, client_hashmap.clone());
            tokio::spawn_async(async move { 
                let (send_job, receive_job) = client.take_jobs();
                tokio::spawn_async(send_job);
                receive_job.await;
            });

            send_client_list_to_all(&client_hashmap.lock().unwrap());
        }
    });
}

fn start_server(port: u16) -> Result<TcpListener, Box<dyn std::error::Error>> {
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?; // to-do: add ipv6
    let listener = TcpListener::bind(&addr)?;
    Ok(listener)
}

fn get_list_of_clients(client_hashmap: &MutexGuard<'_, HashMap<SocketAddr, UnboundedSender<Command>>>) -> String {

    let mut client_list = String::new();
    for key in client_hashmap.keys() {
        if !client_list.is_empty() {
            client_list.push_str(",");
        }
        client_list.push_str(&key.to_string());
    }

    client_list
}

fn send_client_list_to_all(client_hashmap: &MutexGuard<'_, HashMap<SocketAddr, UnboundedSender<Command>>>) {
    for sender in client_hashmap.values() {
        sender.unbounded_send(Command::UpdateClientList {client_list: get_list_of_clients(&client_hashmap)}).unwrap();
    }
}