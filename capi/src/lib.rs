#![feature(await_macro, async_await)]

extern crate libsynchro;

use libc::c_char;
use std::ffi::CStr;
use std::net::SocketAddr;
use libsynchro::SynchroConnection;

#[no_mangle]
pub extern fn synchro_connect(addr: *const c_char, port: u16) -> *mut SynchroConnection {
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
pub extern fn synchro_run_connection(ptr: *mut SynchroConnection)
{
    assert!(!ptr.is_null());
    let connection = unsafe {
        Box::from_raw(ptr)
    };

    connection.run();
}

#[allow(dead_code)]
pub extern fn fix_linking_when_not_using_stdlib() { panic!() }