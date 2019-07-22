#![feature(async_await)]

use libsynchro::*;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::net::SocketAddr;

use futures::sync::mpsc::{self, UnboundedSender};

use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

type ClientHashmapArc = Arc<Mutex<HashMap<SocketAddr, UnboundedSender<Command>>>>;
type ClientNamesHashmapArc = Arc<Mutex<HashMap<SocketAddr, String>>>;

struct Client {
    synchro_conn: Arc<Mutex<SynchroConnection>>,
    addr: SocketAddr,
    client_hashmap: ClientHashmapArc,
    client_names_hashmap: ClientNamesHashmapArc
}

impl Client {
    fn new(socket: TcpStream, client_hashmap: ClientHashmapArc, client_names_hashmap: ClientNamesHashmapArc) -> Self {
        let addr = socket.peer_addr().unwrap();
        println!("Client connected: {}", addr);

        let (sender, mut reciever) = mpsc::unbounded();
        client_hashmap.lock().unwrap().insert(addr, sender);

        let callback_client_hashmap = client_hashmap.clone();
        let callback_client_names_hashmap = client_names_hashmap.clone();
        let callback = move |cmd: Command| {

            // Determine which clients to forward command to
            match cmd {
                Command::UpdateClientList {client_list: _} => {

                    let new_cmd = Command::UpdateClientList {client_list: get_list_of_clients(&callback_client_hashmap, &callback_client_names_hashmap)};

                    callback_client_hashmap.lock().unwrap().get(&addr).unwrap().unbounded_send(new_cmd).unwrap();
                },
                Command::SetName {desired_name} => {
                    println!("{}", desired_name);
                    callback_client_names_hashmap.lock().unwrap().insert(addr, desired_name);
                    send_client_list_to_all(&callback_client_hashmap, &callback_client_names_hashmap);
                },
                _ => {
                    for (peer_addr, sender) in callback_client_hashmap.lock().unwrap().iter() {
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
            client_names_hashmap,
        }
    }

    fn take_jobs(&mut self) -> (AsyncJob, AsyncJob) {
        self.synchro_conn.lock().unwrap().take_jobs()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        println!("Dropped client {}", self.addr);
        self.client_hashmap.lock().unwrap().remove(&self.addr);
        send_client_list_to_all(&self.client_hashmap, &self.client_names_hashmap);
    }
}

fn main() {
    let listener = start_server(32019).unwrap();
    println!("Server started sucessfully on {}", listener.local_addr().unwrap());

    let client_hashmap: ClientHashmapArc = Arc::new(Mutex::new(HashMap::new()));
    let client_names_hashmap: ClientNamesHashmapArc = Arc::new(Mutex::new(HashMap::new()));

    tokio::run_async(async move {
        let mut incoming = listener.incoming();

        while let Some(stream) = incoming.next().await {
            let stream = stream.unwrap();

            let mut client = Client::new(stream, client_hashmap.clone(), client_names_hashmap.clone());
            tokio::spawn_async(async move { 
                let (send_job, receive_job) = client.take_jobs();
                tokio::spawn_async(send_job);
                receive_job.await;
            });

            send_client_list_to_all(&client_hashmap, &client_names_hashmap);
        }
    });
}

fn start_server(port: u16) -> Result<TcpListener, Box<dyn std::error::Error>> {
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?; // to-do: add ipv6
    let listener = TcpListener::bind(&addr)?;
    Ok(listener)
}

fn get_list_of_clients(client_hashmap: &ClientHashmapArc, client_names_hashmap: &ClientNamesHashmapArc) -> String {
    let clients = client_hashmap.lock().unwrap();
    let names = client_names_hashmap.lock().unwrap();

    let mut client_list = String::new();
    for key in clients.keys() {
        if !client_list.is_empty() {
            client_list.push_str(",");
        }

        let mut name_or_key = &key.to_string();
        if let Some(name) = names.get(key) {
            name_or_key = name;
        }
        client_list.push_str(name_or_key);
    }

    client_list
}

fn send_client_list_to_all(client_hashmap: &ClientHashmapArc, client_names_hashmap: &ClientNamesHashmapArc) {
    let retrieved_list = get_list_of_clients(client_hashmap, client_names_hashmap);
    let clients = client_hashmap.lock().unwrap();
    for sender in clients.values() {
        sender.unbounded_send(Command::UpdateClientList { client_list: retrieved_list.clone() } ).unwrap();
    }
}