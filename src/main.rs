use anyhow::*;
use futures::io::{copy, AsyncReadExt, AsyncWriteExt};
use futures::stream::StreamExt;
use log::*;
use smol::Async;

use std::net;

const PORT: u16 = 5000;

async fn handle_client(mut stream: Async<net::TcpStream>) -> Result<()> {
    info!("Request from {:?}", stream);

    // Open a TCP connection to the website
    let tunnel = {
        // Read the CONNECT request
        let mut buf = [0; 4096];
        let nbytes = stream.read(&mut buf).await?;

        let req = std::str::from_utf8(&buf[..nbytes])?;
        info!("Received request\n{}", req);

        let website = req
            .split_whitespace()
            .nth(1)
            .ok_or(anyhow!("invalid request"))?;

        info!("Connecting to {}", website);
        Async::<net::TcpStream>::connect(website).await?
    };

    // Send an acknowledgement to the client
    stream
        .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
        .await?;

    // Keep copying data back and forth
    futures::future::try_join(
        copy(&mut &stream, &mut &tunnel),
        copy(&mut &tunnel, &mut &stream),
    )
    .await?;

    Ok(())
}

fn main() {
    env_logger::init();

    smol::run(async {
        // Create a server
        let server =
            Async::<net::TcpListener>::bind(format!("127.0.0.1:{}", PORT)).unwrap_or_else(|e| {
                if e.kind() == std::io::ErrorKind::AddrInUse {
                    error!("Port {} is already being used by another program", PORT);
                    std::process::exit(1);
                } else {
                    panic!("{:?}", e);
                }
            });
        info!(
            "Listening on port {}",
            server.get_ref().local_addr().unwrap().port()
        );

        // Handle incoming connections
        server
            .incoming()
            .for_each_concurrent(None, |ev| async move {
                if let Ok(stream) = ev {
                    if let Err(error) = handle_client(stream).await {
                        error!("error while handling stream: {}", error);
                    }
                }
            })
            .await;
    })
}
