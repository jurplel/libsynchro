#![feature(await_macro, async_await)]

extern crate libsynchro;

use std::ffi::CStr;
use std::ffi::c_void;
use std::os::raw::c_char;
use std::net::SocketAddr;

use libsynchro::{Command, SynchroConnection};

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Context(*const c_void);

unsafe impl Send for Context {}

#[no_mangle]
pub extern fn synchro_connection_connect(addr: *const c_char, port: u16) -> *mut SynchroConnection {
    let addr = unsafe {
        assert!(!addr.is_null());
        CStr::from_ptr(addr)
    };

    let addr = addr.to_str().unwrap();
    let addr: SocketAddr = format!("{}:{}", addr, port).parse().unwrap();
    let synchro_connection = SynchroConnection::connect(addr).unwrap();
    Box::into_raw(Box::new(synchro_connection))
}

#[no_mangle]
pub extern fn synchro_connection_run(ptr: *mut SynchroConnection) {
    let connection = unsafe {
        assert!(!ptr.is_null());
        Box::from_raw(ptr)
    };

    connection.run();
}

#[no_mangle]
pub extern fn synchro_connection_send(ptr: *mut SynchroConnection, bytes: *const c_char) {
    let connection = unsafe {
        assert!(!ptr.is_null());
        &mut *ptr
    };

    let bytes = unsafe {
        assert!(!bytes.is_null());
        CStr::from_ptr(bytes).to_bytes()
    };

    connection.send(bytes).unwrap();
}

#[no_mangle]
pub extern fn synchro_connection_set_callback(ptr: *mut SynchroConnection, func: fn(Context, Command), ctx: Context) {
    let connection = unsafe {
        assert!(!ptr.is_null());
        &mut *ptr
    };

    let callback = move |cmd: Command| {
        func(ctx, cmd);
    };

    connection.set_callback(Box::new(callback));
}