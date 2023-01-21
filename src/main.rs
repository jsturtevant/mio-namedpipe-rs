// use std::io::Read;
// use std::io::{Read, Write};

use mio::windows::NamedPipe;

use mio::{Events, Interest, Poll, Token};
use std::sync::Arc;
use std::ffi::c_void;
use std::io::{self, Read, Write};

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{FromRawHandle, RawHandle, AsRawHandle};
use std::thread;
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

    let mut server = PipeServer::new(PIPE_NAME);

    loop{
        let con = server.Accept().unwrap();

        let h = start_handler(con);
    }
}

fn start_handler<T: Connection+ 'static> (con: T) -> thread::JoinHandle<()>
//where A: Connection + Send + Sync + 'static
 {
    let newconnection = con;
    
    let h = thread::spawn(move || {
        let mut connection = newconnection;
        let mut count = 0;
        loop {
            let mut buf = [0; 10];
            connection.read(&mut buf);

            count += 1;
            let message = format!("pong-{value}", value = count);
            match connection.write(message.as_bytes()) {
                Ok(_) => println!("Wrote to pipe"),
                Err(e) => break,
            }

            if count == 10 {
                print!("killing client connection");
                io::stdout().flush();
                connection.close();
                break;
            }
        }

        print!("disconnecting");
        io::stdout().flush();
    });

    h
}

// do I do clone and Arc's here?
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

    fn new_instance(&mut self) -> io::Result<pipeinstance> {
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
    
            Ok(pipeinstance { namedPipe: np, poll: Poll::new().unwrap() })
        }
    }
}

struct pipeinstance {
    namedPipe: NamedPipe,
    poll: Poll,
}

impl Read for pipeinstance {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.poll.registry()
                .reregister(&mut self.namedPipe, SERVER, Interest::READABLE)
                .unwrap();

        let mut events = Events::with_capacity(1024);
        self.poll.poll(&mut events, None).unwrap();

        loop {
            match self.namedPipe.read(buf) {
                Ok(0) => {
                    return Err(io::Error::new(io::ErrorKind::Other, "pipe closed"));
                }
                Ok(x) => {
                    print!("read: {:?}", std::str::from_utf8(&buf));
                    
                    return Ok(x);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    self.poll.poll(&mut events, None).unwrap();
                    continue;
                }
                Err(e) => {
                    println!("Error reading from pipe: {}", e);
                    return Err(e)
                }
            }
        }
    }
}

impl Write for pipeinstance {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.poll.registry()
        .reregister(&mut self.namedPipe, SERVER, Interest::WRITABLE)
        .unwrap();

        loop {
            match self.namedPipe.write(buf) {
                Ok(x) => {
                    println!("Wrote to pipe");
                    return Ok(x)
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    println!("blocked");
                    let mut events = Events::with_capacity(1024);
                    self.poll.poll(&mut events, None).unwrap();
                }
                Err(e) if e.raw_os_error() == Some(ERROR_NO_DATA as i32) => {
                    return Err(e)
                }
                Err(e) => {
                    println!("Error writing to pipe: {}", e);
                    return Err(e)
                }
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
    

impl Close for pipeinstance {
    fn close(&mut self) -> io::Result<()> {
        return self.namedPipe.disconnect();
    }
}

impl Connection for pipeinstance {}

impl Listener for PipeServer<'_> {
    type Type = pipeinstance;
    fn Accept(&mut self) -> Result<Self::Type, io::Error> {
        let mut pipe = self.new_instance().unwrap();

        pipe.poll.registry()
        .register(&mut pipe.namedPipe, SERVER, Interest::WRITABLE)
        .unwrap();

        println!("waiting for connection....");
        loop {
            match pipe.namedPipe.connect() {
                Ok(()) => {
                    println!("Server Connected!");
                    return Ok(pipe);
                },
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    pipe.poll.registry()
                        .reregister(&mut pipe.namedPipe, SERVER, Interest::WRITABLE)
                        .unwrap();
    
                    let mut events = Events::with_capacity(1024);
                    pipe.poll.poll(&mut events, None).unwrap();
                }
                Err(e) => {
                    println!("Error connecting to pipe: {}", e);
                    return Err(e);
                }
            }
        }
    }
}

trait Listener {
    type Type: Connection;
    fn Accept(&mut self) -> Result<Self::Type, io::Error>;
}

trait Close {
    fn close(&mut self) -> io::Result<()>;
}

trait Connection: Close + Read + Write + Send + std::marker::Sync {}
