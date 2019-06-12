#![feature(await_macro, async_await)]

extern crate libsynchro;

use std::ffi::CStr;
use std::ffi::c_void;
use std::os::raw::c_char;
use std::net::SocketAddr;

use libsynchro::{Command, SynchroConnection};

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Context(*mut c_void);

unsafe impl Send for Context {}

#[no_mangle]
pub extern fn synchro_connection_new(addr: *const c_char, port: u16, func: fn(Context, Command), ctx: Context) -> *mut SynchroConnection {
    let addr = unsafe {
        assert!(!addr.is_null());
        CStr::from_ptr(addr)
    };

    let addr = addr.to_str().unwrap();
    let addr: SocketAddr = format!("{}:{}", addr, port).parse().unwrap();

    let callback = move |cmd: Command| {
        func(ctx, cmd);
    };

    let synchro_connection = SynchroConnection::new(addr, Box::new(callback)).unwrap();
    Box::into_raw(Box::new(synchro_connection))
}

#[no_mangle]
pub extern fn synchro_connection_free(ptr: *mut SynchroConnection) {
    let mut connection = unsafe {
        assert!(!ptr.is_null());
        Box::from_raw(ptr)
    };
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
pub extern fn synchro_connection_send(ptr: *mut SynchroConnection, cmd: Command) {
    let connection = unsafe {
        assert!(!ptr.is_null());
        &mut *ptr
    };

    connection.send(cmd).unwrap();
}