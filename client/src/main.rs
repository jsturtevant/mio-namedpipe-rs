use mio::windows::NamedPipe;
use std::io::{Write, Read};
use std::os::windows::fs::OpenOptionsExt;
use std::fs::OpenOptions;
use std::os::windows::io::{FromRawHandle, IntoRawHandle};

use windows_sys::Win32::Storage::FileSystem::{
     FILE_FLAG_OVERLAPPED,
     OPEN_EXISTING
};

use windows_sys::Win32::Foundation::{
    ERROR_BROKEN_PIPE, ERROR_PIPE_NOT_CONNECTED
};
use mio::{Events, Interest, Poll, Token};
use std::error::Error;
use std::time::Duration;


fn main() -> Result<(), Box<dyn Error>>{
    println!("Hello, client!");

    let mut opts = OpenOptions::new();
    opts.read(true)
        .write(true)
        .custom_flags(FILE_FLAG_OVERLAPPED);
    let file = opts.open(r"\\.\pipe\mio-named-pipe-test")?;
    let mut client = unsafe { NamedPipe::from_raw_handle(file.into_raw_handle()) };

    let mut poll = Poll::new()?;
    poll
        .registry()
        .register(&mut client, Token(1), Interest::WRITABLE)?;
       
    let mut events = Events::with_capacity(128);

    poll.poll(&mut events, Some(Duration::new(1, 0)))?;
    match client.write(b"ping") {
        Ok(_) => println!("Wrote to pipe"),
        Err(e) => println!("Error writing to pipe: {}", e),
    }

    'outer: loop {
        poll.poll(&mut events, Some(Duration::new(1, 0)))?;

        for event in events.iter() {
            
            // We can use the token we previously provided to `register` to
            // determine for which socket the event is.
            match event.token() {
                Token(1) => {
                    if event.is_readable() {
                        println!("readable! {:?}", event);
                        let mut buf = [0; 10];
                        match client.read(&mut buf) {
                            Ok(0) =>
                            {
                                println!("Read zero from pipe");
                                break 'outer;
                            },
                            Ok(_) => {
                                // print string representation of buffer
                                println!("Read from pipe: {:?}", std::str::from_utf8(&buf));
                            },
                            
                            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_NOT_CONNECTED as i32) =>
                            {
                                println!("no process: {}", e);

                                break 'outer;
                            },
                            Err(e) => println!("Error reading from pipe: {}", e),
                        }
                        
                       
                    }
                    
                }
                _ => panic!("unexpected token"),
            }
        }

        events.clear()
    }

    Ok(())
}
