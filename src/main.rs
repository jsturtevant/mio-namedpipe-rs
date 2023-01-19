// use std::io::Read;
// use std::io::{Read, Write};

use mio::windows::NamedPipe;

use mio::{Events, Interest, Poll, Token};
use std::error::Error;
use std::ffi::c_void;
use std::io::{Read, Write, self};
use std::time::Duration;
use std::thread;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES,
};
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};

use windows_sys::Win32::Storage::FileSystem::{
    ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
};
use windows_sys::Win32::Foundation::{
    ERROR_BROKEN_PIPE, ERROR_IO_INCOMPLETE, ERROR_IO_PENDING, ERROR_NO_DATA, ERROR_PIPE_CONNECTED,
    ERROR_PIPE_LISTENING, HANDLE, INVALID_HANDLE_VALUE,
};

const PIPE_NAME: &str = r"\\.\pipe\mio-named-pipe-test";
const SERVER: Token = Token(0);

fn main()  {
    println!("Hello, server!");

    let mut server = new(PIPE_NAME, true, std::ptr::null_mut()).unwrap();
    
    loop {
        server = waitForConnection(server).unwrap();
    }
}

fn waitForConnection(server: NamedPipe) -> io::Result<NamedPipe> {
    println!("waiting for connection....");

    let mut currentHandle = server;
    let mut poll = Poll::new().unwrap();
    poll.registry().register(
        &mut currentHandle,
        SERVER,
        Interest::WRITABLE 
    ).unwrap();
    let mut events = Events::with_capacity(128);
   
    loop {
        match currentHandle.connect() {
            Ok(()) =>{
                println!("Server Connected!");
      
                thread::spawn(move || {
                    let mut currentServer = currentHandle;
                    poll.registry().reregister(&mut currentServer, SERVER, Interest::READABLE |  Interest::WRITABLE).unwrap();

                    poll.poll(&mut events, Some(Duration::new(10, 0))).unwrap();

                    let mut buf = [0; 10];
                    currentServer.read(&mut buf);
                    print!("read: {:?}", buf);
                
                    match currentServer.write(b"1234"){
                        Ok(_) => println!("Wrote to pipe"),
                        Err(e) => println!("Error writing to pipe: {}", e),
                    }
                
                    let mut events = Events::with_capacity(128);
                    poll.poll(&mut events, Some(Duration::new(1, 0))).unwrap();
                
                    match currentServer.write(b"test end") {
                        Ok(_) => {
                            println!("Wrote to pipe2");
                        },
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            println!("waiting for client!");
                        },
                        Err(e) => {
                            println!("Error writing to pipe: {}", e);
                        } 
                    }
                
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    currentServer.disconnect();

                    print!("disconnecting");
                    io::stdout().flush();
                });

                // create a new one and loop for create again
                return new(PIPE_NAME, false, std::ptr::null_mut());
            },
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                poll.registry().reregister(
                    &mut currentHandle,
                    SERVER,
                    Interest::WRITABLE 
                ).unwrap();

                poll.poll(&mut events,None).unwrap();
            },
            Err(e) => {
                println!("Error connecting to pipe: {}", e);
                return Err(e)
            } 
        }
    }
}

fn new<A: AsRef<OsStr>>(addr: A, first: bool, attrs: *mut c_void) -> io::Result<NamedPipe> {
    let name: Vec<_> = addr.as_ref().encode_wide().chain(Some(0)).collect();

    // bitwise or file_flag_first_pipe_instance with file_flag_overlapped and pipe_access_duplex
    let mut openmode = PIPE_ACCESS_DUPLEX |  FILE_FLAG_OVERLAPPED;
    if first {
        openmode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
    }


    // Safety: syscall
    let h = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            openmode,
            PIPE_TYPE_BYTE,
            PIPE_UNLIMITED_INSTANCES,
            65536,
            65536,
            0,
            std::ptr::null_mut(),
        )
    };

    if h == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        // Safety: nothing actually unsafe about this. The trait fn includes
        // `unsafe`.
        Ok(unsafe { NamedPipe::from_raw_handle(h as RawHandle) })
    }
}