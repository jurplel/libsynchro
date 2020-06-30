use std::net::SocketAddr;
use std::sync::Arc;
use std::net::Shutdown;
use bytes::{Buf, BufMut, Bytes, BytesMut};

use futures_channel::mpsc;

use async_std::prelude::*;
use async_std::net::TcpStream;
use async_std::task;

use serde::Deserialize;

#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    Invalid,
    Pause { paused: bool, percent_pos: f64 },
    Seek { percent_pos: f64, dragged: bool },
    UpdateClientList { client_list: String },
    SetName { desired_name: String },
    SetCurrentFile { file_size: u64, file_duration: f64, file_name: String},
}

impl Command {
    pub fn to_u8(&self) -> u8 {
        match self {
            Command::Pause { .. } => 1,
            Command::Seek { .. } => 2,
            Command::UpdateClientList { .. } => 3,
            Command::SetName { .. } => 4,
            Command::SetCurrentFile {..} => 5,
            _ => 0,
        }
    }

    pub fn into_bytes(self) -> Bytes {
        let mut body = vec![];

        body.put_u8(self.to_u8());

        match self {
            Command::Pause {
                paused,
                percent_pos,
            } => {
                body.put_u8(paused as u8);
                body.put_f64(percent_pos);
            }
            Command::Seek {
                percent_pos,
                dragged,
            } => {
                body.put_f64(percent_pos);
                body.put_u8(dragged as u8);
            }
            Command::UpdateClientList { client_list } => {
                body.put(Bytes::from(client_list));
            }
            Command::SetName { desired_name } => {
                body.put(Bytes::from(desired_name));
            }
            Command::SetCurrentFile { file_size, file_duration, file_name } => {
                body.put_u64(file_size);
                body.put_f64(file_duration);
                body.put(Bytes::from(file_name));
            }
            _ => {}
        }

        let mut message = vec![];
        message.put_u16(body.len() as u16);
        message.append(&mut body);
        Bytes::from(message)
    }

    pub fn from_buf(mut data: impl Buf) -> Self {
        let numeric_command = data.get_u8();

        let command = match numeric_command {
            1 => {
                let paused = data.get_u8() != 0;
                let percent_pos = data.get_f64();
                Command::Pause {
                    paused,
                    percent_pos,
                }
            }
            2 => {
                let percent_pos = data.get_f64();
                let dragged = data.get_u8() != 0;
                Command::Seek {
                    percent_pos,
                    dragged,
                }
            }
            3 => {
                let client_list = String::from_utf8(data.bytes().to_vec()).unwrap();
                data.advance(client_list.len());
                Command::UpdateClientList { client_list }
            }
            4 => {
                let desired_name = String::from_utf8(data.bytes().to_vec()).unwrap();
                data.advance(desired_name.len());
                Command::SetName { desired_name }
            }
            5 => {
                let file_size = data.get_u64();
                let file_duration = data.get_f64();
                let file_name = String::from_utf8(data.bytes().to_vec()).unwrap();
                data.advance(file_name.len());
                Command::SetCurrentFile { file_size, file_duration, file_name }
            }
            _ => Command::Invalid,
        };

        if data.has_remaining() {
            println!(
                "Warning: {} left over bytes from {:?} ({})",
                data.remaining(),
                command,
                numeric_command
            )
        };

        command
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Event {
    CommandReceived { command: Command },
}

pub type NewCallbackFn = Arc<dyn Fn(Result<SynchroConnection, std::io::Error>) + Sync + Send>;
pub type CallbackFn = Arc<dyn Fn(Event) + Sync + Send>;

pub struct SynchroConnection {
    stream: Box<TcpStream>,
    unbounded_sender: Option<mpsc::UnboundedSender<Bytes>>,
    callback: Option<CallbackFn>
}


impl SynchroConnection {
    pub fn new(addr: SocketAddr, cb: NewCallbackFn) {
        task::spawn(async move {
            let stream_result = TcpStream::connect(&addr).await;
            match stream_result {
                Ok(stream) => cb(Ok(SynchroConnection::from_existing(stream))),
                Err(e) => cb(Err(e)),
            }
        });
    }

    pub fn new_blocking(addr: SocketAddr) -> Result<Self, Box<dyn std::error::Error>> {
        let connection = task::block_on(TcpStream::connect(&addr))?;

        Ok(SynchroConnection::from_existing(connection))
    }

    pub async fn new_async(addr: SocketAddr) -> Result<Self, Box<dyn std::error::Error>> {
        let connection = TcpStream::connect(&addr).await?;

        Ok(SynchroConnection::from_existing(connection))
    }

    pub fn set_callback(&mut self, callback: CallbackFn) {
        self.callback = Some(callback);
    }

    pub fn from_existing(stream: TcpStream) -> Self {
        let stream = Box::new(stream);
        let callback = None;


        SynchroConnection {
            stream,
            unbounded_sender: None,
            callback,
        }
    }

    pub fn run(&mut self) {
        let (unbounded_sender, unbounded_receiver) = mpsc::unbounded::<Bytes>();
        self.unbounded_sender = Some(unbounded_sender);
        
        task::spawn(receive_data(self.stream.clone(), self.callback.clone()));
        task::spawn(send_data(self.stream.clone(), unbounded_receiver));

        self.unbounded_sender = None;
    }

    pub fn send(
        &mut self,
        command: Command,
    ) -> Result<(), mpsc::TrySendError<Bytes>> {
        println!("Sending {:?}", command);
        if let Some(sender) = &self.unbounded_sender {
            sender.unbounded_send(command.into_bytes())?;
        }
        Ok(())
    }

    pub fn destroy(&mut self) -> Result<(), mpsc::TrySendError<Bytes>> {
        if let Some(sender) = &self.unbounded_sender {
            sender.unbounded_send(Bytes::new())?;
        }
        Ok(())
    }
}

impl Drop for SynchroConnection {
    fn drop(&mut self) {
        println!("Dropped connection");
    }
}

pub async fn receive_data(mut stream: Box<TcpStream>, callback: Option<CallbackFn>) -> Result<(), Box<std::io::Error>> {
    loop {
        // 2 bytes for u16
        let mut buf = BytesMut::with_capacity(2);
        stream.read_exact(buf.as_mut()).await?;
        let anticipated_message_length = buf.get_u16();

        let mut buf = BytesMut::with_capacity(anticipated_message_length as usize);
        stream.read_exact(buf.as_mut()).await?;
        let command = Command::from_buf(buf);
        
        println!("Received {:?}", command);

        if let Some(cb) = &callback {
            (&cb)(Event::CommandReceived { command });
        }
    }
}

pub async fn send_data(mut stream: Box<TcpStream>, mut unbounded_receiver: mpsc::UnboundedReceiver<Bytes>) -> Result<(), Box<std::io::Error>> {
    while let Some(data) = unbounded_receiver.next().await {
        if data.is_empty() {
            break;
        }
        stream.write(&data).await?;
    }
    stream.shutdown(Shutdown::Both)?;
    Ok(())
}


#[derive(Deserialize)]
struct SynchroJsonData {
    servers: Vec<Server>,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct Server {
    pub name: String,
    pub ip: String,
}

pub fn get_server_list(url: Option<&str>) -> Result<Vec<Server>, surf::Error> {
    let url = url.unwrap_or("https://interversehq.com/synchro/synchro.json");

    let body: SynchroJsonData = task::block_on(surf::get(url).recv_json())?;

    Ok(body.servers)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
