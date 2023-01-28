use mio::windows::NamedPipe;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{FromRawHandle, IntoRawHandle};

use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OVERLAPPED;

use mio::{Events, Interest, Poll, Token};
use std::error::Error;
use windows_sys::Win32::Foundation::ERROR_PIPE_NOT_CONNECTED;

fn main() -> Result<(), Box<dyn Error>> {
    println!("Hello, client!");

    let mut opts = OpenOptions::new();
    opts.read(true)
        .write(true)
        .custom_flags(FILE_FLAG_OVERLAPPED);
    let file = opts.open(r"\\.\pipe\mio-named-pipe-test")?;
    let mut client = unsafe { NamedPipe::from_raw_handle(file.into_raw_handle()) };

    let mut poll = Poll::new()?;
    poll.registry()
        .register(&mut client, Token(1), Interest::WRITABLE)?;

    let mut events = Events::with_capacity(128);

    // initiate ping/pong
    poll.poll(&mut events, None)?;
    match client.write(b"ping") {
        Ok(_) => println!("Wrote to pipe"),
        Err(e) => println!("Error writing to pipe: {}", e),
    }

    let mut count = 0;
    'outer: loop {
        poll.registry()
            .reregister(&mut client, Token(1), Interest::READABLE)?;
        poll.poll(&mut events, None).unwrap();

        let mut buf = [0; 6];
        match client.read(&mut buf) {
            Ok(0) => {
                println!("Read zero from pipe");
                continue;
            }
            Ok(_) => {
                println!("Read from pipe: {:?}", std::str::from_utf8(&buf));
            }
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_NOT_CONNECTED as i32) => {
                println!("no process: {}", e);

                break 'outer;
            }
            Err(e) => println!("Error reading from pipe: {}", e),
        }

        std::thread::sleep(std::time::Duration::from_secs(1));
        poll.registry()
            .reregister(&mut client, Token(1), Interest::WRITABLE)
            .unwrap();
        count += 1;
        loop {
            let message = format!("ping-{value}", value = count);
            match client.write(message.as_bytes()) {
                Ok(_) => {
                    println!("Wrote to pipe");
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    poll.poll(&mut events, None).unwrap();
                }
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_NOT_CONNECTED as i32) => {
                    println!("no process: {}", e);

                    break 'outer;
                }
                Err(e) => {
                    println!("Error writing to pipe: {}", e);
                }
            }
        }
    }

    Ok(())
}
