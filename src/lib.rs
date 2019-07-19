#![feature(async_await)]

mod wrappedtcpstream;
use wrappedtcpstream::WrappedTcpStream;

use std::net::SocketAddr;
use std::pin::Pin;

use futures::sync::mpsc;

use bytes::{Bytes, BytesMut, IntoBuf, Buf, BufMut};

use tokio::net::TcpStream;
use tokio::codec::{BytesCodec, FramedRead, FramedWrite};
use tokio::prelude::*;

#[repr(C)]
#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    Invalid,
    Pause {paused: bool, percent_pos: f64},
    Seek {percent_pos: f64, dragged: bool},
    UpdateClientList {client_list: String},
}

impl Command {
    pub fn to_u8(&self) -> u8 {
        match self {
            Command::Pause {paused: _, percent_pos: _} => 1,
            Command::Seek {percent_pos: _, dragged: _} => 2,
            Command::UpdateClientList {client_list: _} => 3,
            _ => 0,
        }
    }

    pub fn into_bytes(self) -> Bytes {
        let mut body = vec![];

        body.put_u8(self.to_u8());

        match self {
            Command::Pause {paused, percent_pos} => { 
                body.put_u8(paused as u8);
                body.put_f64_be(percent_pos);
            },
            Command::Seek {percent_pos, dragged} => {
                body.put_f64_be(percent_pos);
                body.put_u8(dragged as u8);
            },
            Command::UpdateClientList {client_list} => {
                body.put(client_list.into_bytes());
            }
            _ => {},
        }

        let mut message = vec![];
        message.put_u16_be(body.len() as u16);
        message.append(&mut body);
        Bytes::from(message)
    }

    pub fn from_buf(mut data: impl Buf) -> Self {
        let numeric_command = data.get_u8();

        let command = match numeric_command {
            1 => {
                let paused = data.get_u8() != 0;
                let percent_pos = data.get_f64_be();
                Command::Pause {paused, percent_pos}
            },
            2 => {
                let percent_pos = data.get_f64_be();
                let dragged = data.get_u8() != 0;
                Command::Seek {percent_pos, dragged}
            },
            3 => {
                let client_list = String::from_utf8(data.bytes().to_vec()).unwrap();
                data.advance(client_list.len());
                Command::UpdateClientList {client_list}
            }
            _ => Command::Invalid,
        };

        if data.has_remaining() {
            println!("Warning: {} bytes not used from recieved {:?}", data.remaining(), command)
        };

    command
    }
}

pub type CallbackFn = Box<dyn Fn(Command) + Send>;
pub type AsyncJob = Pin<Box<dyn std::future::Future<Output = ()> + Send>>;

pub struct SynchroConnection {
    unbounded_sender: mpsc::UnboundedSender<Bytes>,
    send_job: Option<AsyncJob>,
    receive_job: Option<AsyncJob>,
}

impl SynchroConnection {
    pub fn new(addr: SocketAddr, callback: CallbackFn) -> Result<Self, Box<dyn std::error::Error>> {
        let connection = TcpStream::connect(&addr).wait()?;
        Ok(SynchroConnection::from_existing(connection, callback))
    }

    pub fn from_existing(socket: TcpStream, callback: CallbackFn) -> Self {
        let wrapped = WrappedTcpStream(socket);

        let (read_half, write_half) = wrapped.split();

        let mut stream = FramedRead::new(read_half, BytesCodec::new());
        let mut sink = FramedWrite::new(write_half, BytesCodec::new());

        let (unbounded_sender, mut unbounded_receiver) = mpsc::unbounded::<Bytes>();

        // Define send job
        let send_job = async move {
            while let Some(data) = unbounded_receiver.next().await {
                let data = data.unwrap();
                // An empty bytes object is treated as a disconnect signal
                if data.len() == 0 { break; }
                sink.send_async(data).await.unwrap();
            }
            sink.get_mut().shutdown().unwrap();
        };

        let send_job = Box::pin(send_job);

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
                        
                        anticipated_message_length = buffer.split_to(2).into_buf().get_u16_be();

                        println!("message length: {}", anticipated_message_length);
                    }

                    // Process the rest of the message if we're sure that we have the entire thing
                    if buffer.len() < anticipated_message_length as usize {
                        break;
                    }
                    
                    let split_bytes = buffer.split_to(anticipated_message_length as usize);

                    // Call user-provided callback to handle received commands
                    (callback.as_ref())(Command::from_buf(split_bytes.into_buf()));

                    anticipated_message_length = 0;
                }
            }
        };

        let receive_job = Box::pin(receive_job);

        SynchroConnection {
            unbounded_sender,
            send_job: Some(send_job),
            receive_job: Some(receive_job),
        }
    }

    pub fn run(&mut self) {
        let (send_job, receive_job) = self.take_jobs();
        tokio::run_async(async move {
            tokio::spawn_async(send_job);
            receive_job.await;
        });
    }

    pub fn take_jobs(&mut self) -> (AsyncJob, AsyncJob) {
        let send_job = self.send_job.take().unwrap();
        let receive_job = self.receive_job.take().unwrap();
        (send_job, receive_job)
    }

    pub fn send(&self, command: Command) -> Result<(), mpsc::SendError<Bytes>> {
        self.unbounded_sender.unbounded_send(command.into_bytes())
    }

    pub fn destroy(&mut self) -> Result<(), mpsc::SendError<Bytes>> {
        self.unbounded_sender.unbounded_send(Bytes::new())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
