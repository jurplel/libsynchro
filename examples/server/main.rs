#![feature(async_await)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::net::SocketAddr;

use futures::sync::mpsc;

use bytes::{Bytes, BytesMut, IntoBuf, Buf, BufMut};

use tokio::net::{TcpListener, TcpStream};
use tokio::codec::{Framed, BytesCodec, Decoder};
use tokio::prelude::stream::*;
use tokio::prelude::*;

type ClientListArc = Arc<Mutex<HashMap<SocketAddr, mpsc::UnboundedSender<Bytes>>>>;

struct Client {
    stream: SplitStream<Framed<TcpStream, BytesCodec>>,
    addr: SocketAddr,
    client_list: ClientListArc,
}

impl Client {
    fn new(socket: TcpStream, client_list: ClientListArc) -> Self {
        let addr = socket.peer_addr().unwrap();
        println!("Client connected: {}", addr);

        let framed = BytesCodec::new().framed(socket);
        let (mut sink, stream) = framed.split();

        let (sender, mut reciever) = mpsc::unbounded();
        client_list.lock().unwrap().insert(addr, sender);

        // When data is sent to this Client object through the
        // mpsc system, send it to the actual client
        tokio::spawn_async(async move { 
            while let Some(data) = reciever.next().await {
                let data = data.unwrap();
                println!("{:?}", data);
                sink.send_async(data).await.unwrap();
            }
        });

        Client {
            stream,
            addr,
            client_list,
        }
    }

    async fn recieve(&mut self) { 
        let mut buffer = BytesMut::new();
        let mut anticipated_message_length: u16 = 0;
        while let Some(message) = self.stream.next().await {
            // Add newly received information to the buffer
            buffer.unsplit(message.unwrap());
            
            // Retrieve message length if we did'nt already
            if anticipated_message_length == 0 {
                if buffer.len() < 2 {
                    continue;
                }
                
                anticipated_message_length = buffer.split_to(2).into_buf().get_u16_be();

                println!("message length: {}", anticipated_message_length);
            }

            // Process the rest of the message if we're sure that we have the entire thing
            if buffer.len() >= anticipated_message_length as usize {
                let split_bytes = buffer.split_to(anticipated_message_length as usize);
                let mut header_bytes = BytesMut::with_capacity(2);
                header_bytes.put_u16_be(anticipated_message_length);
                header_bytes.unsplit(split_bytes.clone());

                let mut clients = self.client_list.lock().unwrap();
                for (peer_addr, sender) in clients.iter_mut() {
                    if *peer_addr != self.addr {
                        sender.unbounded_send(Bytes::from(header_bytes.clone())).unwrap();
                    }
                }

                anticipated_message_length = 0;
                buffer.clear(); // remember to remove this
            }
        }
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

            tokio::spawn_async(async move { 
                client.recieve().await; 
            });
        }
    });
}

fn start_server(port: u16) -> Result<TcpListener, Box<dyn std::error::Error>> {
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse()?; // to-do: add ipv6
    let listener = TcpListener::bind(&addr)?;
    Ok(listener)
}