use futures_util::{SinkExt, StreamExt};
use log::*;
use std::{net::SocketAddr, time::Duration};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::Error as TTError;
use tokio_tungstenite::tungstenite::Result as TTResult;
use tungstenite::error::CapacityError::MessageTooLong;

async fn accept_connection(peer: SocketAddr, stream: TcpStream) {
    if let Err(e) = handle_connection(peer, stream).await {
        match e {
            TTError::ConnectionClosed | TTError::Protocol(_) | TTError::Utf8 => (),
            err => error!("Error processing connection: {}", err),
        }
    }
}

async fn handle_connection(peer: SocketAddr, stream: TcpStream) -> TTResult<()> {
    let ws_stream = accept_async(stream).await.expect("Failed to accept");
    info!("New WebSocket connection: {}", peer);
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    loop {
        tokio::select! {
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(t))) => {
                        info!("text: {:?}", t);
                        let v: serde_json::Result<serde_json::Value> = serde_json::from_str(t.as_str());
                        match v {
                            Ok(o) => {
                                debug!("json: {:?}", o);
                            },
                            Err(err) => {
                                debug!("error: {:?}", err);
                            }
                        }
                        continue;
                    },
                    Some(Ok(Message::Binary(_))) => {
                        info!("binary");
                        continue;
                    },
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {
                        info!("ping/pong");
                        continue;
                    }
                    Some(Err(TTError::Capacity(MessageTooLong{size, max_size}))) => {
                        info!("size");
                        continue;
                    },
                    None |
                    Some(Ok(Message::Close(_)) |
                         Err(TTError::AlreadyClosed | TTError::ConnectionClosed |
                             TTError::Protocol(tungstenite::error::ProtocolError::ResetWithoutClosingHandshake)))
                        => {
                            info!("close or error");
                            break;
                        },
                    Some(Err(TTError::Io(e))) => {
                        // IO errors are considered fatal
                        warn!("io error: {:?}", e);
                        break;
                    }
                    x => {
                        // default condition on error is to close the client connection
                        info!("unknown: {:?}", x);
                        break;
                    }
                }
            }
            // _ = interval.tick() => {
            //     ws_sender.send(Message::Text("tick".to_owned())).await?;
            // }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let addr = "127.0.0.1:9002";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    info!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream.peer_addr().expect("connected streams should have a peer address");
        info!("Peer address: {}", peer);

        tokio::spawn(accept_connection(peer, stream));
    }
}
