// use std::io::Read;
// use std::io::{Read, Write};

use mio::windows::NamedPipe;

use mio::{Events, Interest, Poll, Token};

use std::ffi::c_void;
use std::io::{self, Read, Write};

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{FromRawHandle, RawHandle};
use std::thread;
use windows_sys::Win32::System::Pipes::{
    CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES,
};

use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
};

const PIPE_NAME: &str = r"\\.\pipe\mio-named-pipe-test";
const SERVER: Token = Token(0);

fn main() {
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
    poll.registry()
        .register(&mut currentHandle, SERVER, Interest::WRITABLE)
        .unwrap();
    let mut events = Events::with_capacity(128);

    loop {
        match currentHandle.connect() {
            Ok(()) => {
                println!("Server Connected!");

                thread::spawn(move || {
                    let mut currentServer = currentHandle;

                    let mut count = 0;
                    loop {
                        poll.registry()
                            .reregister(&mut currentServer, SERVER, Interest::READABLE)
                            .unwrap();
                        poll.poll(&mut events, None).unwrap();

                        let mut buf = [0; 10];
                        match currentServer.read(&mut buf) {
                            Ok(0) => {
                                continue;
                            }
                            Ok(_) => {
                                print!("read: {:?}", std::str::from_utf8(&buf));
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                continue;
                            }
                            Err(e) => {
                                println!("Error reading from pipe: {}", e);
                            }
                        }

                        poll.registry()
                            .reregister(&mut currentServer, SERVER, Interest::WRITABLE)
                            .unwrap();

                        count += 1;
                        loop {
                            let message = format!("pong-{value}", value = count);
                            match currentServer.write(message.as_bytes()) {
                                Ok(_) => {
                                    println!("Wrote to pipe");
                                    break;
                                }
                                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                    poll.poll(&mut events, None).unwrap();
                                }
                                Err(e) => {
                                    println!("Error writing to pipe: {}", e);
                                }
                            }
                        }

                        if count == 10 {
                            print!("killing client connection");
                            io::stdout().flush();
                            currentServer.disconnect();
                            drop(currentServer);
                            break;
                        }
                    }

                    print!("disconnecting");
                    io::stdout().flush();
                });

                // create a new one and loop for create again
                return new(PIPE_NAME, false, std::ptr::null_mut());
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                poll.registry()
                    .reregister(&mut currentHandle, SERVER, Interest::WRITABLE)
                    .unwrap();

                poll.poll(&mut events, None).unwrap();
            }
            Err(e) => {
                println!("Error connecting to pipe: {}", e);
                return Err(e);
            }
        }
    }
}

fn new<A: AsRef<OsStr>>(addr: A, first: bool, _attrs: *mut c_void) -> io::Result<NamedPipe> {
    let name: Vec<_> = addr.as_ref().encode_wide().chain(Some(0)).collect();

    // bitwise or file_flag_first_pipe_instance with file_flag_overlapped and pipe_access_duplex
    let mut openmode = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED;
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
