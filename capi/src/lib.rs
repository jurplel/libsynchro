#![feature(await_macro, async_await)]

extern crate libsynchro;

use std::ffi::CStr;
use std::ffi::CString;
use std::ffi::c_void;
use std::os::raw::c_char;
use std::net::SocketAddr;

use libsynchro::{Command, SynchroConnection};

#[repr(C)]
#[derive(Debug)]
pub enum Synchro_Command {
    Invalid,
    Pause {paused: bool, percent_pos: f64},
    Seek {percent_pos: f64, dragged: bool},
    UpdateClientList {client_list: *mut c_char},
}

impl Synchro_Command {
    fn from_command(cmd: Command) -> Self {
        match cmd {
            Command::Invalid => Synchro_Command::Invalid,
            Command::Pause {paused, percent_pos} => Synchro_Command::Pause {paused, percent_pos},
            Command::Seek {percent_pos, dragged} => Synchro_Command::Seek {percent_pos, dragged},
            Command::UpdateClientList {client_list} => {
                let client_list = CString::new(client_list).unwrap().into_raw();
                Synchro_Command::UpdateClientList {client_list}
            },
        } 
    }

    fn into_command(self) -> Command {
        match self {
            Synchro_Command::Invalid => Command::Invalid,
            Synchro_Command::Pause {paused, percent_pos} => Command::Pause {paused, percent_pos},
            Synchro_Command::Seek {percent_pos, dragged} => Command::Seek {percent_pos, dragged},
            Synchro_Command::UpdateClientList {client_list} => { 
                Command::UpdateClientList {
                    client_list: unsafe {
                        assert!(!client_list.is_null()); 
                        CStr::from_ptr(client_list).to_str().unwrap().to_string()
                    }
                }
            },
        }
    }
}

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Context(*mut c_void);

unsafe impl Send for Context {}

#[no_mangle]
pub extern fn synchro_connection_new(addr: *const c_char, port: u16, func: fn(Context, Synchro_Command), ctx: Context) -> *mut SynchroConnection {
    let addr = unsafe {
        assert!(!addr.is_null());
        CStr::from_ptr(addr)
    };

    let addr = addr.to_str().unwrap();
    let addr: SocketAddr = format!("{}:{}", addr, port).parse().unwrap();

    let callback = move |cmd: Command| {
        func(ctx, Synchro_Command::from_command(cmd));
    };

    let result = SynchroConnection::new(addr, Box::new(callback));
    match result {
        Ok(connection) => Box::into_raw(Box::new(connection)),
        Err(error) => {
            println!("Error: {}", error);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern fn synchro_connection_free(ptr: *mut SynchroConnection) {
    let mut connection = unsafe {
        assert!(!ptr.is_null());
        Box::from_raw(ptr)
    };
    connection.destroy().unwrap();
}

#[no_mangle]
pub extern fn synchro_connection_run(ptr: *mut SynchroConnection) {
    let connection = unsafe {
        assert!(!ptr.is_null());
        &mut *ptr
    };

    connection.run();
}

#[no_mangle]
pub extern fn synchro_connection_send(ptr: *mut SynchroConnection, cmd: Synchro_Command) {
    let connection = unsafe {
        assert!(!ptr.is_null());
        &mut *ptr
    };

    connection.send(cmd.into_command()).unwrap();
}

#[no_mangle]
pub extern fn synchro_char_free(ptr: *mut c_char) {
    unsafe {
        CString::from_raw(ptr);
    }
}