#![feature(await_macro, async_await)]

use std::net::SocketAddr;

use bytes::{BytesMut, IntoBuf, Buf};

use tokio::net::TcpStream;
use tokio::codec::{Framed, BytesCodec, Decoder};
use tokio::prelude::*;

#[derive(Debug, Hash, PartialEq, Eq)]
pub enum Command {
    Invalid = 0,
    Pause = 1,
    Seek = 2,
}

impl From<u8> for Command {
    fn from(num: u8) -> Self {
        match num {
            1 => Command::Pause,
            2 => Command::Seek,
            _ => Command::Invalid,
        }
    }
}

pub struct SynchroConnection {
    framed: Framed<TcpStream, BytesCodec>,
}

impl SynchroConnection {
    pub fn connect(addr: SocketAddr) -> Result<Self, Box<std::error::Error>> {
        let socket = TcpStream::connect(&addr).wait()?;
        let framed = BytesCodec::new().framed(socket);

        Ok(SynchroConnection {
            framed,
        })
    }

    pub async fn recieve(&mut self) {
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
