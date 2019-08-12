// This is a helper to allow shutting down the stream from only the write half.
// It was originally made by dremon on Github
// https://github.com/tokio-rs/tokio/issues/852#issuecomment-459766144

// use std::net::Shutdown;

// use bytes::{Buf, BufMut};

// use tokio::net::TcpStream;
// use tokio::io;
// use tokio::prelude::*;

// pub struct WrappedTcpStream(pub TcpStream);

// impl Read for WrappedTcpStream {
//     fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
//         self.0.read(buf)
//     }
// }

// impl Write for WrappedTcpStream {
//     fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
//         self.0.write(buf)
//     }

//     fn flush(&mut self) -> io::Result<()> {
//         self.0.flush()
//     }
// }

// impl AsyncRead for WrappedTcpStream {
//     unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [u8]) -> bool {
//         self.0.prepare_uninitialized_buffer(buf)
//     }

//     fn read_buf<B: BufMut>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
//         self.0.read_buf(buf)
//     }
// }

// impl AsyncWrite for WrappedTcpStream {
//     fn shutdown(&mut self) -> Poll<(), io::Error> {
//         self.0.shutdown(Shutdown::Write)?;
//         Ok(().into())
//     }

//     fn write_buf<B: Buf>(&mut self, buf: &mut B) -> Poll<usize, io::Error> {
//         self.0.write_buf(buf)
//     }
// }