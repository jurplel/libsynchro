use libsynchro::*;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::stream::StreamExt;

struct ClientInfo {
    name: Option<String>,
    file_size: Option<u64>,
    file_duration: Option<f64>,
    file_name: Option<String>,
}

type ClientHashmapArc = Arc<Mutex<HashMap<SocketAddr, mpsc::UnboundedSender<Command>>>>;
type ClientInfoHashmapArc = Arc<Mutex<HashMap<SocketAddr, ClientInfo>>>;

struct Client {
    synchro_conn: Arc<Mutex<SynchroConnection>>,
    addr: SocketAddr,
    client_hashmap: ClientHashmapArc,
    client_info_hashmap: ClientInfoHashmapArc,
}

impl Client {
    fn new(
        socket: TcpStream,
        client_hashmap: ClientHashmapArc,
        client_info_hashmap: ClientInfoHashmapArc,
    ) -> Self {
        let addr = socket.peer_addr().unwrap();
        println!("Client connected: {}", addr);

        let (sender, mut reciever) = mpsc::unbounded_channel();
        client_hashmap.lock().unwrap().insert(addr, sender);
        client_info_hashmap.lock().unwrap().insert(addr, ClientInfo { name: None, file_size: None, file_duration: None, file_name: None});

        let callback_client_hashmap = client_hashmap.clone();
        let callback_client_info_hashmap = client_info_hashmap.clone();
        let callback = move |cmd: Command| {
            // Execute special instructions for received command, or otherwise simply forward to other clients
            match cmd {
                Command::UpdateClientList { .. } => {
                    let new_cmd = Command::UpdateClientList {
                        client_list: get_list_of_clients(
                            &callback_client_hashmap,
                            &callback_client_info_hashmap,
                        )
                        .unwrap(),
                    };

                    callback_client_hashmap
                        .lock()
                        .unwrap()
                        .get_mut(&addr)
                        .unwrap()
                        .send(new_cmd)
                        .unwrap();
                }
                Command::SetName { desired_name } => {
                    println!("{}", desired_name);
                    callback_client_info_hashmap.lock().unwrap().get_mut(&addr).unwrap().name = Some(desired_name);
                    send_client_list_to_all(&callback_client_hashmap, &callback_client_info_hashmap).unwrap();
                }
                Command::SetCurrentFile { file_size, file_duration, file_name } => {
                    callback_client_info_hashmap.lock().unwrap().get_mut(&addr).unwrap().file_size = Some(file_size);
                    callback_client_info_hashmap.lock().unwrap().get_mut(&addr).unwrap().file_duration = Some(file_duration);
                    callback_client_info_hashmap.lock().unwrap().get_mut(&addr).unwrap().file_name = Some(file_name);
                    send_client_list_to_all(&callback_client_hashmap, &callback_client_info_hashmap).unwrap();
                }
                _ => {
                    for (peer_addr, sender) in callback_client_hashmap.lock().unwrap().iter_mut() {
                        if *peer_addr != addr {
                            sender.send(cmd.clone()).unwrap();
                        }
                    }
                }
            }
        };

        let synchro_conn = Arc::new(Mutex::new(SynchroConnection::from_existing(
            socket,
            Box::new(callback),
            None
        )));

        let synchro_conn2 = synchro_conn.clone();

        tokio::spawn(async move {
            while let Some(cmd) = reciever.next().await {
                let cmd = cmd;
                synchro_conn2.lock().unwrap().send(cmd).unwrap();
            }
        });

        Client {
            synchro_conn,
            addr,
            client_hashmap,
            client_info_hashmap,
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
        send_client_list_to_all(&self.client_hashmap, &self.client_info_hashmap).unwrap();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listener = start_server(32019).await?;
    println!("Server started sucessfully on {}", listener.local_addr()?);

    let client_hashmap: ClientHashmapArc = Arc::new(Mutex::new(HashMap::new()));
    let client_info_hashmap: ClientInfoHashmapArc = Arc::new(Mutex::new(HashMap::new()));
    let mut incoming = listener.incoming();

    while let Some(stream) = incoming.next().await {
        let stream = stream?;

        let mut client = Client::new(stream, client_hashmap.clone(), client_info_hashmap.clone());
        tokio::spawn(async move {
            let (send_job, receive_job) = client.take_jobs();
            tokio::spawn(send_job);
            receive_job.await;
        });

        send_client_list_to_all(&client_hashmap, &client_info_hashmap).unwrap();
    }
    Ok(())
}

async fn start_server(port: u16) -> Result<TcpListener, Box<dyn std::error::Error>> {
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?; // to-do: add ipv6
    let listener = TcpListener::bind(&addr).await?;
    Ok(listener)
}

fn get_list_of_clients(
    client_hashmap: &ClientHashmapArc,
    client_info_hashmap: &ClientInfoHashmapArc,
) -> Result<String, Box<dyn std::error::Error>> {
    let clients = client_hashmap.lock().unwrap();
    let infos = client_info_hashmap.lock().unwrap();

    let mut client_list = String::new();
    for key in clients.keys() {
        if !client_list.is_empty() {
            client_list.push_str(",");
        }

        let string_key = key.to_string();
        let mut name_or_key = &string_key;

        let name = infos.get(key).unwrap().name.as_ref().unwrap_or(&string_key);

        if !name.is_empty() {
            name_or_key = name;
        }

        client_list.push_str(name_or_key);
        
        client_list.push_str(";");
        client_list.push_str(&infos.get(key).unwrap().file_size.unwrap_or(0).to_string());
        client_list.push_str(";");
        client_list.push_str(&infos.get(key).unwrap().file_duration.unwrap_or(0.0).to_string());
        client_list.push_str(";");
        client_list.push_str(infos.get(key).unwrap().file_name.as_ref().unwrap_or(&String::new()));
    }

    Ok(client_list)
}

fn send_client_list_to_all(
    client_hashmap: &ClientHashmapArc,
    client_info_hashmap: &ClientInfoHashmapArc,
) -> Result<(), Box<dyn std::error::Error>> {
    let retrieved_list = get_list_of_clients(client_hashmap, client_info_hashmap)?;
    let mut clients = client_hashmap.lock().unwrap();
    for sender in clients.values_mut() {
        sender.send(Command::UpdateClientList {
            client_list: retrieved_list.clone(),
        })?;
    }
    Ok(())
}
