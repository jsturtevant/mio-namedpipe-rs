# mio-namedpipe-rs

sample application to learn rust, mio and namedpipes

Run the server:

```powershell
cargo run

Running `target\debug\np-windows-rust.exe`     
Hello, server!
waiting for connection....
```

Run client (even multiple at same time):

```powershell
 cargo run -p client
  Running `target\debug\client.exe`
Hello, client!
Wrote to pipe
Read from pipe: Ok("pong-1\0\0\0\0")
Wrote to pipe
Read from pipe: Ok("pong-2\0\0\0\0")
Wrote to pipe
```

### helpful commands

```
cargo clippy --fix --all
cargo fmt --all
```