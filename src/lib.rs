use std::net::SocketAddr;
use std::pin::Pin;

use futures::sink::SinkExt;

use bytes::{Buf, BufMut, Bytes, BytesMut};

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::runtime::Runtime;
use tokio::stream::StreamExt;
use tokio::prelude::*;

use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite};

use serde::Deserialize;

#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    Invalid,
    Pause { paused: bool, percent_pos: f64 },
    Seek { percent_pos: f64, dragged: bool },
    UpdateClientList { client_list: String },
    SetName { desired_name: String },
}

impl Command {
    pub fn to_u8(&self) -> u8 {
        match self {
            Command::Pause { .. } => 1,
            Command::Seek { .. } => 2,
            Command::UpdateClientList { .. } => 3,
            Command::SetName { .. } => 4,
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
            _ => Command::Invalid,
        };

        if data.has_remaining() {
            println!(
                "Warning: {} bytes not used from recieved {:?}",
                data.remaining(),
                command
            )
        };

        command
    }
}

pub type CallbackFn = Box<dyn Fn(Command) + Send>;
pub type AsyncJob = Pin<Box<dyn std::future::Future<Output = ()> + Send>>;

pub struct SynchroConnection {
    unbounded_sender: mpsc::UnboundedSender<Bytes>,
    runtime: Option<Runtime>,
    send_job: Option<AsyncJob>,
    receive_job: Option<AsyncJob>,
}

impl SynchroConnection {
    pub fn new(addr: SocketAddr, callback: CallbackFn) -> Result<Self, Box<dyn std::error::Error>> {
        let mut runtime = Runtime::new().unwrap();
        let connection = runtime.block_on(TcpStream::connect(&addr))?;

        Ok(SynchroConnection::from_existing(connection, callback, Some(runtime)))
    }

    pub async fn new_async(addr: SocketAddr, callback: CallbackFn) -> Result<Self, Box<dyn std::error::Error>> {
        let connection = TcpStream::connect(&addr).await?;

        Ok(SynchroConnection::from_existing(connection, callback, None))
    }

    pub fn from_existing(socket: TcpStream, callback: CallbackFn, runtime: Option<Runtime>) -> Self {
        let (read_half, write_half) = tokio::io::split(socket);

        let mut stream = FramedRead::new(read_half, BytesCodec::new());
        let mut sink = FramedWrite::new(write_half, BytesCodec::new());

        let (unbounded_sender, mut unbounded_receiver) = mpsc::unbounded_channel::<Bytes>();

        // Define send job
        let send_job = async move {
            while let Some(data) = unbounded_receiver.next().await {
                // An empty bytes object is treated as a disconnect signal
                if data.is_empty() {
                    break;
                }
                sink.send(data).await.unwrap();
            }
            sink.get_mut().shutdown();
        };

        // Define receive job
        let receive_job = async move {
            let mut buffer = BytesMut::new();
            let mut anticipated_message_length: u16 = 0;
            while let Some(message) = stream.next().await {
                // Add newly received information to the buffer
                buffer.unsplit(message.unwrap());

                loop {
                    // Retrieve message length if we didn't already
                    if anticipated_message_length == 0 {
                        if buffer.len() < 2 {
                            break;
                        }

                        anticipated_message_length = buffer.split_to(2).get_u16();
                    }

                    // Process the rest of the message if we're sure that we have the entire thing
                    if buffer.len() < anticipated_message_length as usize {
                        break;
                    }

                    let split_bytes = buffer.split_to(anticipated_message_length as usize);

                    let command = Command::from_buf(split_bytes);

                    println!("{:?}", command);

                    // Call user-provided callback to handle received commands
                    (callback.as_ref())(command);

                    anticipated_message_length = 0;
                }
            }
        };

        SynchroConnection {
            unbounded_sender,
            runtime,
            send_job: Some(Box::pin(send_job)),
            receive_job: Some(Box::pin(receive_job)),
        }
    }

    pub fn run(&mut self) {
        assert!(self.runtime.is_some(), "Runtime not found; You must pass a runtime to from_existing to use the run function");
        let (send_job, receive_job) = self.take_jobs();
        self.runtime.take().unwrap().block_on(async move {
            tokio::spawn(receive_job);
            send_job.await;
        });
    }

    pub fn take_jobs(&mut self) -> (AsyncJob, AsyncJob) {
        let send_job = self.send_job.take().unwrap();
        let receive_job = self.receive_job.take().unwrap();
        (send_job, receive_job)
    }

    pub fn send(
        &mut self,
        command: Command,
    ) -> Result<(), mpsc::error::SendError<Bytes>> {
        self.unbounded_sender.send(command.into_bytes())
    }

    pub fn destroy(&mut self) -> Result<(), mpsc::error::SendError<Bytes>> {
        self.unbounded_sender.send(Bytes::new())
    }
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

pub fn get_server_list(url: Option<&str>) -> Result<Vec<Server>, Box<dyn std::error::Error>> {
    let url = url.unwrap_or("https://interversehq.com/synchro/synchro.json");

    let body: SynchroJsonData = reqwest::get(url)?.json()?;

    Ok(body.servers)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
