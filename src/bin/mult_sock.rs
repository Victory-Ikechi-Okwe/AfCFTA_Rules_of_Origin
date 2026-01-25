use std::error::Error;
use std::path::Path;
use std::process;
use tokio::{
    self,
    io::{AsyncReadExt, AsyncWriteExt},
    net::unix::OwnedWriteHalf,
    net::UnixListener,
    net::UnixStream,
    sync::mpsc,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Define paths for read and write sockets
    let read_socket = Path::new("/tmp/read.sock");
    let write_socket = Path::new("/tmp/write.sock");

    // Remove existing read socket if it exists
    if read_socket.exists() {
        std::fs::remove_file(read_socket)?;
    }

    // Create the Unix domain socket listener
    let listener = UnixListener::bind(read_socket)?;
    println!("Server listening on {:?}", read_socket);

    // Try to connect to the write socket
    let write_stream = UnixStream::connect(write_socket).await?;

    // Split the write stream into read and write halves
    let (mut read_half, write_half) = write_stream.into_split();

    // Create a channel for sending processed data to the writer task
    let (tx, rx) = mpsc::channel(32);

    // Spawn a task to monitor the write end for disconnection
    tokio::spawn(async move {
        let mut buffer = [0u8; 1];
        match read_half.read(&mut buffer).await {
            Ok(0) => {
                println!("Write socket disconnected (EOF), shutting down...");
                process::exit(0);
            }
            Ok(_) => {
                eprintln!("Unexpected data received on write socket");
                process::exit(1);
            }
            Err(e) => {
                eprintln!("Write socket error: {}", e);
                process::exit(1);
            }
        }
    });

    // Spawn a task to handle writing to the output socket
    tokio::spawn(handle_writer(write_half, rx));

    // Handle incoming connections
    loop {
        let (read_stream, _) = listener.accept().await?;
        println!("Accepted new connection");

        let tx = tx.clone();

        // Spawn a new task to handle this connection
        tokio::spawn(async move {
            handle_connection(read_stream, tx).await;
        });
    }
}

// Handle a single client connection
async fn handle_connection(stream: UnixStream, tx: mpsc::Sender<Vec<u8>>) {
    let (mut read_half, _) = stream.into_split();
    let mut buffer = [0u8; 1024];

    loop {
        match read_half.read(&mut buffer).await {
            Ok(n) if n == 0 => {
                println!("Client disconnected");
                break;
            }
            Ok(n) => {
                // Process the received data
                let received_data = &buffer[..n];
                let processed_data = process_data(received_data);

                // Send the processed data to the writer task
                if let Err(e) = tx.send(processed_data).await {
                    eprintln!("Failed to send processed data: {}", e);
                    break;
                }
            }
            Err(e) => {
                eprintln!("Failed to read from socket: {}", e);
                break;
            }
        }
    }
}

// Handle writing to the output socket
async fn handle_writer(mut write_half: OwnedWriteHalf, mut rx: mpsc::Receiver<Vec<u8>>) {
    while let Some(data) = rx.recv().await {
        if let Err(e) = write_half.write_all(&data).await {
            eprintln!("Failed to write to socket: {}", e);
            break;
        }

        if let Err(e) = write_half.flush().await {
            eprintln!("Failed to flush socket: {}", e);
            break;
        }
    }
}

// Example data processing function
fn process_data(input: &[u8]) -> Vec<u8> {
    // Example: Convert to uppercase if it's valid UTF-8
    if let Ok(str_data) = String::from_utf8(input.to_vec()) {
        str_data.to_uppercase().into_bytes()
    } else {
        // If not valid UTF-8, return the original data
        input.to_vec()
    }
}
