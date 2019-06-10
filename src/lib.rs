#![feature(await_macro, async_await)]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use bytes::{BytesMut, IntoBuf, Buf};

use tokio::net::TcpStream;
use tokio::codec::{Framed, BytesCodec, Decoder};
use tokio::prelude::*;

#[repr(C)]
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Command {
    Invalid,
    Pause {paused: bool, percent_pos: f64},
    Seek {percent_pos: f64, dragged: bool},
}

// impl From<u8> for Command {
//     fn from(num: u8) -> Self {
//         match num {
//             1 => Command::Pause,
//             2 => Command::Seek,
//             _ => Command::Invalid,
//         }
//     }
// }

type ClosureType = Box<Fn(Command) + Send>;

pub struct SynchroConnection {
    framed: Framed<TcpStream, BytesCodec>,
    callback: ClosureType,
}

impl SynchroConnection {
    pub fn connect(addr: SocketAddr) -> Result<Self, Box<std::error::Error>> {
        let socket = TcpStream::connect(&addr).wait()?;
        let framed = BytesCodec::new().framed(socket);

        let callback = Box::new(|_| {});

        Ok(SynchroConnection {
            framed,
            callback,
        })
    }

    pub fn set_callback(&mut self, func: ClosureType) {
        self.callback = func;
    }

    pub fn run(mut self) {
        tokio::run_async(async move {
            await!(self.recieve());
        });
    }

    async fn recieve(&mut self) {
        let mut buffer = BytesMut::new();
        let mut anticipated_message_length: u16 = 0;
        while let Some(message) = await!(self.framed.next()) {
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
                let split_ref = split_bytes.as_ref();
                let split_buffer = std::io::Cursor::new(split_ref);

                anticipated_message_length = 0;
                buffer.clear(); // remember to remove this
                
                (self.callback)(Command::Invalid);
            }
        }
    }
}


// fn handle_data(mut data: std::io::Cursor<&[u8]>) {
//     let has_arguments = data.get_u8() != 0;
//     println!("{}", has_arguments);

//     let numeric_command = data.get_u8();
//     let command = Command::from(numeric_command);
    

//     match command {
//         Command::Invalid => {
//             println!("Invalid commnad recieved");
//         }
//         Command::Pause => {
//             let paused = data.get_u8() != 0;
//             let percent_pos = data.get_f64_be();
//         }
//         Command::Seek => {
//             let percent_pos = data.get_f64_be();
//             let dragged = data.get_u8() != 0;
//         }
//     }
// }

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
