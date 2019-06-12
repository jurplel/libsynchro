#![feature(async_await)]

use std::net::SocketAddr;
use std::pin::Pin;
use std::io::Cursor;

use futures::sync::mpsc;

use bytes::{Bytes, BytesMut, IntoBuf, Buf, BufMut};

use tokio::net::TcpStream;
use tokio::codec::{Framed, BytesCodec, Decoder};
use tokio::prelude::stream::*;
use tokio::prelude::*;

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Command {
    Invalid,
    Pause {paused: bool, percent_pos: f64},
    Seek {percent_pos: f64, dragged: bool},
}

impl Command {
    pub fn to_u8(&self) -> u8 {
        match self {
            Command::Pause {paused: _, percent_pos: _} => 1,
            Command::Seek {percent_pos: _, dragged: _} => 2,
            _ => 0,
        }
    }
}

type CallbackClosure = Box<dyn Fn(Command) + Send>;

pub struct SynchroConnection {
    stream: SplitStream<Framed<TcpStream, BytesCodec>>,
    unbounded_sender: mpsc::UnboundedSender<Bytes>,
    send_job: Option<Pin<Box<dyn std::future::Future<Output = ()> + Send>>>,
    callback: Option<CallbackClosure>,
}

impl SynchroConnection {
    pub fn connect(addr: SocketAddr) -> Result<Self, Box<dyn std::error::Error>> {
        let socket = TcpStream::connect(&addr).wait()?;

        let framed = BytesCodec::new().framed(socket);
        let (mut sink, stream) = framed.split();

        let (unbounded_sender, mut unbounded_receiver) = mpsc::unbounded::<Bytes>();

        let send_job = async move {
            while let Some(data) = unbounded_receiver.next().await {
                let data = data.unwrap();
                sink.send_async(data).await.unwrap();
            }
        };

        let send_job = Box::pin(send_job);

        let callback = None;

        Ok(SynchroConnection {
            stream,
            unbounded_sender,
            send_job: Some(send_job),
            callback,
        })
    }

    pub fn set_callback(&mut self, func: CallbackClosure) {
        self.callback = Some(func);
    }

    pub fn run(mut self) {
        let send_job = self.send_job.take().unwrap();
        tokio::run_async(async move {
            tokio::spawn_async(send_job);
            self.receive().await;
        });
    }

    pub fn send(&mut self, command: Command) -> Result<(), mpsc::SendError<Bytes>> {
        let mut bytes = vec![];

        bytes.put_u8(command.to_u8());

        match command {
            Command::Pause {paused, percent_pos} => { 
                bytes.put_u8(paused as u8);
                bytes.put_f64_be(percent_pos);
            },
            Command::Seek {percent_pos, dragged} => {
                bytes.put_f64_be(percent_pos);
                bytes.put_u8(dragged as u8);
            },
            _ => println!("Sending invalid command"),
        }

        let mut header = vec![];
        header.put_u16_be(bytes.len() as u16);
        header.append(&mut bytes);

        self.unbounded_sender.unbounded_send(Bytes::from(header))
    }

    async fn receive(&mut self) {
        let mut buffer = BytesMut::new();
        let mut anticipated_message_length: u16 = 0;
        while let Some(message) = self.stream.next().await {
            // Add newly received information to the buffer
            buffer.unsplit(message.unwrap());
            
            // Retrieve message length if we didn't already
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
                let split_buf = Cursor::new(split_bytes.as_ref());

                (self.callback.as_ref().unwrap())(handle_data(split_buf));

                anticipated_message_length = 0;
                buffer.clear();
            }
        }
    }
}

fn handle_data(mut data: impl Buf) -> Command {
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
        _ => { 
            println!("Recieved invalid command");
            Command::Invalid
        },
    };

    if data.has_remaining() {
        println!("Warning: all data not used from recieved {:?}", command)
    };

    command
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
