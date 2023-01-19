// use std::io::Read;
// use std::io::{Read, Write};

use mio::windows::NamedPipe;
use mio::{Events, Interest, Poll, Token};
use std::error::Error;
use std::io::{Read, Write};
use std::time::Duration;
use std::thread;

const SERVER: Token = Token(0);
fn main()  {
    println!("Hello, server!");

    let mut server = NamedPipe::new(r"\\.\pipe\mio-named-pipe-test").unwrap();
    let mut poll = Poll::new().unwrap();
    poll.registry().register(
        &mut server,
        SERVER,
        Interest::WRITABLE 
    ).unwrap();
    let mut events = Events::with_capacity(128);

    waitForConnection(&mut server, &mut poll, &mut events);
        
    poll.poll(&mut events, Some(Duration::new(10, 0))).unwrap();

    let mut buf = [0; 10];
    server.read(&mut buf);
    print!("read: {:?}", buf);

    match server.write(b"1234"){
        Ok(_) => println!("Wrote to pipe"),
        Err(e) => println!("Error writing to pipe: {}", e),
    }

    let mut events = Events::with_capacity(128);
    poll.poll(&mut events, Some(Duration::new(1, 0))).unwrap();

    match server.write(b"test end") {
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
    print!("disconnecting");
    server.disconnect();
    // unregister
    poll.registry().deregister(&mut server).unwrap();

    waitForConnection(&mut server, &mut poll, &mut events);

    let mut events = Events::with_capacity(128);

    poll.poll(&mut events, Some(Duration::new(10, 0))).unwrap();

    let mut buf = [0; 10];
    loop {
        match server.read(&mut buf) {
            Ok(_) => {
                println!("Read from pipe: {:?}", std::str::from_utf8(&buf));
                break;
            },
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                poll.poll(&mut events, Some(Duration::new(10, 0))).unwrap();
            },
            Err(e) => {
                println!("Error reading from pipe: {}", e);
            } 
        }
    }
}

fn waitForConnection(server: &mut NamedPipe, poll: &mut Poll, events: &mut Events) {
    println!("waiting for connection....");
    loop {
        match server.connect() {
            Ok(()) =>{
                 println!("Server Connected!");
                break;
            },
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {

                poll.registry().reregister(
                    server,
                    SERVER,
                    Interest::WRITABLE 
                ).unwrap();

                poll.poll(events, Some(Duration::new(10, 0))).unwrap();
                poll.registry().reregister(server, SERVER, Interest::READABLE | Interest::WRITABLE).unwrap();
            },
            Err(e) => {
                println!("Error connecting to pipe: {}", e);
            } 
        }
    }
}
