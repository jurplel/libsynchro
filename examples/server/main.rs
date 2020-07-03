use libsynchro::{SynchroConnection, Event, Command};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures_channel::mpsc;

use async_std::prelude::*;
use async_std::net::{TcpListener, TcpStream};
use async_std::task;

struct ClientInfo {
    name: Option<String>,
    file_size: Option<u64>,
    file_duration: Option<f64>,
    file_name: Option<String>,
    unbounded_sender: Option<mpsc::UnboundedSender<Command>>,
}

impl ClientInfo {
    fn new_empty() -> Self {
        ClientInfo {
            name: None,
            file_size: None,
            file_duration: None,
            file_name: None,
            unbounded_sender: None,
        }
    }
}

type ClientHashmapArc = Arc<Mutex<HashMap<SocketAddr, ClientInfo>>>;

fn receive_event_from_client(event: Event, client_hashmap: &ClientHashmapArc, addr: &SocketAddr) {
    match event {
        Event::CommandReceived { command: cmd } => {
            // Execute special instructions for received command, or otherwise simply forward to other clients
            match cmd {
                Command::UpdateClientList { .. } => {
                    let new_cmd = Command::UpdateClientList {
                        client_list: get_list_of_clients(
                            &client_hashmap
                        )
                        .unwrap(),
                    };
        
                    client_hashmap
                        .lock()
                        .unwrap()
                        .get_mut(&addr)
                        .unwrap()
                        .unbounded_sender
                        .as_ref()
                        .unwrap()
                        .unbounded_send(new_cmd)
                        .unwrap();
                }
                Command::SetName { desired_name } => {
                    println!("{}", desired_name);
                    client_hashmap.lock().unwrap().get_mut(addr).unwrap().name = Some(desired_name);
                    send_client_list_to_all(&client_hashmap).unwrap();
                }
                Command::SetCurrentFile { file_size, file_duration, file_name } => {
                    client_hashmap.lock().unwrap().get_mut(addr).unwrap().file_size = Some(file_size);
                    client_hashmap.lock().unwrap().get_mut(addr).unwrap().file_duration = Some(file_duration);
                    client_hashmap.lock().unwrap().get_mut(addr).unwrap().file_name = Some(file_name);
                    send_client_list_to_all(&client_hashmap).unwrap();
                }
                _ => {
                    for (peer_addr, info) in client_hashmap.lock().unwrap().iter_mut() {
                        if peer_addr != addr {
                            info.unbounded_sender.as_ref().unwrap().unbounded_send(cmd.clone()).unwrap();
                        }
                    }
                }
            }
        }
    }
}

async fn send_cmd_to_client(synchro_conn: Arc<Mutex<SynchroConnection>>, mut unbounded_receiver: mpsc::UnboundedReceiver<Command>) {  
    while let Some(cmd) = unbounded_receiver.next().await {
        synchro_conn.lock().unwrap().send(cmd).unwrap();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    task::block_on(accept_loop())
}

async fn accept_loop() -> Result<(), Box<dyn std::error::Error>> {
    let listener = start_server(32019).await?;
    println!("Server listening on port {}", listener.local_addr()?.port());

    let client_hashmap: ClientHashmapArc = Arc::new(Mutex::new(HashMap::new()));

    while let Some(stream) = listener.incoming().next().await {
        let stream = stream?;

        client_connected(stream, client_hashmap.clone()).await?;

        send_client_list_to_all(&client_hashmap)?;
    }
    Ok(())
}

async fn client_connected(socket: TcpStream, client_hashmap: ClientHashmapArc) -> Result<(), Box<dyn std::error::Error>> {
    let addr = socket.peer_addr()?;
    println!("Client connected: {}", addr);

    client_hashmap.lock().unwrap().insert(addr, ClientInfo::new_empty());

    let synchro_conn = Arc::new(Mutex::new(SynchroConnection::from_existing(socket)));

    synchro_conn.lock().unwrap().run();

    let callback_client_hashmap = client_hashmap.clone();
    synchro_conn.lock().unwrap().set_callback(Arc::new(move |event: Event| {
        receive_event_from_client(event, &callback_client_hashmap, &addr);
    }));

    let (unbounded_sender, unbounded_receiver) = mpsc::unbounded::<Command>();
    client_hashmap.lock().unwrap().get_mut(&addr).unwrap().unbounded_sender.replace(unbounded_sender);

    task::spawn(send_cmd_to_client(synchro_conn.clone(), unbounded_receiver));

    println!("end here");
    Ok(())
}

async fn start_server(port: u16) -> Result<TcpListener, Box<dyn std::error::Error>> {
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?; // to-do: add ipv6
    let listener = TcpListener::bind(&addr).await?;
    Ok(listener)
}

fn get_list_of_clients(
    client_hashmap: &ClientHashmapArc
) -> Result<String, Box<dyn std::error::Error>> {
    let clients = client_hashmap.lock().unwrap();

    let mut client_list = String::new();
    for key in clients.keys() {
        if !client_list.is_empty() {
            client_list.push_str(",");
        }

        let string_key = key.to_string();
        let mut name_or_key = &string_key;

        let name = clients.get(key).unwrap().name.as_ref().unwrap_or(&string_key);

        if !name.is_empty() {
            name_or_key = name;
        }

        client_list.push_str(name_or_key);
        
        client_list.push_str(";");
        client_list.push_str(&clients.get(key).unwrap().file_size.unwrap_or(0).to_string());
        client_list.push_str(";");
        client_list.push_str(&clients.get(key).unwrap().file_duration.unwrap_or(0.0).to_string());
        client_list.push_str(";");
        client_list.push_str(clients.get(key).unwrap().file_name.as_ref().unwrap_or(&String::new()));
    }

    Ok(client_list)
}

fn send_client_list_to_all(
    client_hashmap: &ClientHashmapArc
) -> Result<(), Box<dyn std::error::Error>> {
    let retrieved_list = get_list_of_clients(client_hashmap)?;
    let mut clients = client_hashmap.lock().unwrap();
    for info in clients.values_mut() {
        let sender = info.unbounded_sender.as_ref().unwrap();
        sender.unbounded_send(Command::UpdateClientList {
            client_list: retrieved_list.clone(),
        })?;
    }
    Ok(())
}