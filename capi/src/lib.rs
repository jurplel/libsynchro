extern crate libsynchro;

use std::sync::Arc;

use std::ffi::c_void;
use std::ffi::CStr;
use std::ffi::CString;
use std::net::SocketAddr;
use std::os::raw::c_char;

use libsynchro::{Command, Event, SynchroConnection};

#[repr(C)]
#[derive(Debug)]
pub enum Synchro_Command {
    Invalid,
    Pause { paused: bool, percent_pos: f64 },
    Seek { percent_pos: f64, dragged: bool },
    UpdateClientList { client_list: *mut c_char },
    SetName { desired_name: *mut c_char },
    SetCurrentFile { file_size: u64, file_duration: f64, file_name: *mut c_char },
}

impl Synchro_Command {
    fn from_command(cmd: Command) -> Self {
        match cmd {
            Command::Invalid => Synchro_Command::Invalid,
            Command::Pause {
                paused,
                percent_pos,
            } => Synchro_Command::Pause {
                paused,
                percent_pos,
            },
            Command::Seek {
                percent_pos,
                dragged,
            } => Synchro_Command::Seek {
                percent_pos,
                dragged,
            },
            Command::UpdateClientList { client_list } => {
                let client_list = CString::new(client_list).unwrap().into_raw();
                Synchro_Command::UpdateClientList { client_list }
            }
            Command::SetName { desired_name } => {
                let desired_name = CString::new(desired_name).unwrap().into_raw();
                Synchro_Command::SetName { desired_name }
            }
            Command::SetCurrentFile { file_size, file_duration, file_name } => {
                let file_name = CString::new(file_name).unwrap().into_raw();
                Synchro_Command::SetCurrentFile { file_size, file_duration, file_name }
            }
        }
    }

    fn into_command(self) -> Command {
        match self {
            Synchro_Command::Invalid => Command::Invalid,
            Synchro_Command::Pause {
                paused,
                percent_pos,
            } => Command::Pause {
                paused,
                percent_pos,
            },
            Synchro_Command::Seek {
                percent_pos,
                dragged,
            } => Command::Seek {
                percent_pos,
                dragged,
            },
            Synchro_Command::UpdateClientList { client_list } => Command::UpdateClientList {
                client_list: unsafe {
                    assert!(!client_list.is_null());
                    CStr::from_ptr(client_list).to_str().unwrap().to_string()
                },
            },
            Synchro_Command::SetName { desired_name } => Command::SetName {
                desired_name: unsafe {
                    assert!(!desired_name.is_null());
                    CStr::from_ptr(desired_name).to_str().unwrap().to_string()
                },
            },
            Synchro_Command::SetCurrentFile { file_size, file_duration, file_name } => Command::SetCurrentFile {
                file_size,
                file_duration,
                file_name: unsafe {
                    assert!(!file_name.is_null());
                    CStr::from_ptr(file_name).to_str().unwrap().to_string()
                }
            }
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub enum Synchro_Event {
    CommandReceived { command: Synchro_Command },
    ConnectionClosed,
}

impl Synchro_Event {
    fn from_event(event: Event) -> Self {
        match event {
            Event::CommandReceived { command } => Synchro_Event::CommandReceived {
                command: Synchro_Command::from_command(command)
            },
            Event::ConnectionClosed => Synchro_Event::ConnectionClosed
        }
    }

    fn into_event(self) -> Event {
        match self {
            Synchro_Event::CommandReceived { command } => Event::CommandReceived {
                command: command.into_command()
            },
            Synchro_Event::ConnectionClosed => Event::ConnectionClosed
        }
    }
}

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Context(*mut c_void);

unsafe impl Sync for Context {}
unsafe impl Send for Context {}

#[no_mangle]
pub unsafe extern fn synchro_connection_new_blocking(addr: *const c_char, port: u16) -> *mut SynchroConnection {
    assert!(!addr.is_null());
    let addr = CStr::from_ptr(addr);

    let result = || -> Result<*mut SynchroConnection, Box<dyn std::error::Error>> {
        let addr = addr.to_str()?;
        let addr: SocketAddr = format!("{}:{}", addr, port).parse()?;

        let conn = SynchroConnection::new_blocking(addr)?;
        Ok(Box::into_raw(Box::new(conn)))
    }();
    result.unwrap_or_else(|error|{
        println!("Error connecting to server: {}", error);
        std::ptr::null_mut()
    })
}

#[no_mangle]
pub unsafe extern fn synchro_connection_new(addr: *const c_char, port: u16, callback: extern fn(*mut SynchroConnection, Context), ctx: Context) {
    assert!(!addr.is_null());
    let addr = CStr::from_ptr(addr);

    let result = || -> Result<(), Box<dyn std::error::Error>> {
        let addr = addr.to_str()?;
        let addr: SocketAddr = format!("{}:{}", addr, port).parse()?;

        let cb = move |conn_result: Result<SynchroConnection, std::io::Error>| {
            match conn_result {
                Ok(conn) => callback(Box::into_raw(Box::new(conn)), ctx),
                Err(e) => println!("Error connecting to server: {}", e),
            }
        };

        SynchroConnection::new(addr, Arc::new(cb));
        Ok(())
    }();
    result.unwrap_or_else(|e| {
        println!("Error connecting to server: {}", e);
    });
}


#[no_mangle]
pub unsafe extern fn synchro_connection_set_callback(ptr: *mut SynchroConnection, callback: extern fn(Synchro_Event, Context), ctx: Context)
{    
    assert!(!ptr.is_null());
    let connection = &mut *ptr;
    let cb = move |event: Event| {
        let event = Synchro_Event::from_event(event);
        println!("Event going to callback {:?}", event);
        callback(event, ctx);
    };

    connection.set_callback(Arc::new(cb));
}

#[no_mangle]
pub unsafe extern fn synchro_connection_free(ptr: *mut SynchroConnection) {
    assert!(!ptr.is_null());
    let mut connection = Box::from_raw(ptr);

    connection.destroy().unwrap();
}

#[no_mangle]
pub unsafe extern fn synchro_connection_run(ptr: *mut SynchroConnection) { 
    assert!(!ptr.is_null());
    let connection = &mut *ptr;

    connection.run();
}

#[no_mangle]
pub unsafe extern fn synchro_connection_send(
    ptr: *mut SynchroConnection,
    cmd: Synchro_Command,
) {
    assert!(!ptr.is_null());
    let connection = &mut *ptr;

    connection.send(cmd.into_command()).unwrap();
}

#[no_mangle]
pub unsafe extern fn synchro_char_free(ptr: *mut c_char) {
    CString::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern fn synchro_get_server_list(url: *const c_char) -> *mut c_char {
    let url = CStr::from_ptr(url);
    let url_slice = url.to_str().unwrap();

    let mut url_option = None;
    if !url_slice.is_empty() {
        url_option = Some(url_slice);
    };

    let server_list = libsynchro::get_server_list(url_option).unwrap();

    let mut server_string = String::new();
    for x in 0..server_list.len() {
        let server = &server_list[x];
        server_string += &server.name;
        server_string += ",";
        server_string += &server.ip;

        // Don't add a semicolon to the end
        if x != &server_list.len()-1 {
            server_string += ";";
        } 
    }

    CString::new(server_string).unwrap().into_raw()
}
