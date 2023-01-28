// use std::io::Read;
// use std::io::{Read, Write};

use mio::event::Event;
use mio::windows::NamedPipe;

use mio::{Events, Interest, Poll, Token, Registry};
use std::collections::HashMap;
use std::sync::Arc;
use std::ffi::c_void;
use std::io::{self, Read, Write};

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{FromRawHandle, RawHandle, AsRawHandle};
use std::thread;
use std::time::Duration;
use windows_sys::Win32::System::Pipes::{
    CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES
};

use windows_sys::Win32::Foundation::{
    INVALID_HANDLE_VALUE, ERROR_NO_DATA
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
};

const PIPE_NAME: &str = r"\\.\pipe\mio-named-pipe-test";
const SERVER: Token = Token(0);

fn main() {
    println!("Hello, server!");

    let mut namedpipe = PipeServer::new(PIPE_NAME);

    let mut server = namedpipe.new_instance().unwrap();


    let mut poll = Poll::new().unwrap();

    let mut events = Events::with_capacity(128);


    poll.registry()
    .register(&mut server, SERVER, Interest::WRITABLE).unwrap();

    let mut connections = HashMap::new();
    // Unique token for each incoming connection.
    let mut unique_token = Token(SERVER.0 + 1);

    loop{
        match server.connect() {
            Ok(()) => {
                println!("Connected to client");
            },
            Err(ref e) if would_block(e) => { 
                // let it process other connections
            }
            Err(e) => {
                panic!("Error connecting to client: {}", e);
            }
        };

        poll.poll(&mut events, None ).unwrap();      
        for event in events.iter() {
            match event.token() {
                SERVER => {             
                    println!("Accepted connection from");
                    // save current connected and create new pipe
                    let mut inner = server;  
                    server = namedpipe.new_instance().unwrap();

                    // register currently connected pipe with new token
                    let token = next(&mut unique_token);
                    poll.registry()
                    .reregister(&mut inner, token, Interest::WRITABLE).unwrap();

                    // register new server with the server token to send it into the connect mode. 
                    poll.registry()
                        .register(&mut server, SERVER, Interest::WRITABLE).unwrap();

                    connections.insert(token, inner);
                },
                token => {
                    let done = if let Some(connection) = connections.get_mut(&token) {
                        match handle_connection_event(poll.registry(), connection, event) {
                            Ok(done) => done,
                            Err(e) => {
                                println!("Pipe closed: {}", e);
                                true // error occurred, so connection is done
                            }
                        }
                    } else {
                        // Sporadic events happen, we can safely ignore them.
                        false
                    };
                    if done {
                        if let Some(mut connection) = connections.remove(&token) {
                            poll.registry().deregister(&mut connection).unwrap();
                        }
                    }
                }
            }
        }
    }
}

fn handle_connection_event(
    registry: &Registry,
    connection: &mut NamedPipe,
    event: &Event,
) -> io::Result<bool> {
    if event.is_writable() {
        // We can (maybe) write to the connection.
        let message = format!("pong-{value}", value = event.token().0);
        match connection.write(message.as_bytes()) {
            Ok(_) => {
                // After we've written something we'll reregister the connection
                // to only respond to readable events.
                registry.reregister(connection, event.token(), Interest::READABLE)?
            }
            // Would block "errors" are the OS's way of saying that the
            // connection is not actually ready to perform this I/O operation.
            Err(ref err) if would_block(err) => {}
            // Got interrupted (how rude!), we'll try again.
            Err(ref err) if interrupted(err) => {
                return handle_connection_event(registry, connection, event)
            }
            // Other errors we'll consider fatal.
            Err(err) => return Err(err),
        }
    }

    if event.is_readable() {
        let mut connection_closed= false;
        let mut buf = [0; 10];
        // We can (maybe) read from the connection.
        loop {
            match connection.read(&mut buf) {
                Ok(0) => {
                    // Reading 0 bytes means the other side has closed the
                    // connection or is done writing, then so are we.
                    connection_closed = true;
                    break;
                }
                Ok(n) => {
                    println!("Read from pipe: {:?}", std::str::from_utf8(&buf));
                }
                // Would block "errors" are the OS's way of saying that the
                // connection is not actually ready to perform this I/O operation.
                Err(ref err) if would_block(err) => break,
                Err(ref err) if interrupted(err) => continue,
                // Other errors we'll consider fatal.
                Err(err) => return Err(err),
            }
        }
        if connection_closed {
            println!("Connection closed");
            return Ok(true);
        }
    }

    Ok(false)
}

fn would_block(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::WouldBlock
}

fn next(current: &mut Token) -> Token {
    let next = current.0;
    current.0 += 1;
    Token(next)
}


fn interrupted(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::Interrupted
}

struct PipeServer<'a> {
    firstInstance: Option<bool>,
    address: &'a str,
}

impl PipeServer<'_> {
    fn new(addr:&str) -> PipeServer {
        PipeServer {
            firstInstance: None,
            address: addr,
        }
    }

    fn new_instance(&mut self) -> io::Result<NamedPipe> {
        let name = OsStr::new(self.address)
            .encode_wide()
            .chain(Some(0)) // add NULL termination
            .collect::<Vec<_>>();
    
        // bitwise or file_flag_first_pipe_instance with file_flag_overlapped and pipe_access_duplex
        let mut openmode = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED;

        match self.firstInstance {
            Some(_) => {},
            None => {
                self.firstInstance = Some(true);
                openmode |= FILE_FLAG_FIRST_PIPE_INSTANCE
            }
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
                std::ptr::null_mut(), // todo set this on first instance
            )
        };
    
        if h == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            // Safety: nothing actually unsafe about this. The trait fn includes
            // `unsafe`.
            let np =unsafe { NamedPipe::from_raw_handle(h as RawHandle) };
    
            Ok(  np)
        }
    }
}