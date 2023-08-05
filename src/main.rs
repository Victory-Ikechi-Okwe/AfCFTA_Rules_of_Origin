use futures_util::{StreamExt};
use log::*;
use std::{net::SocketAddr};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
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


#[derive(Debug)]
enum ActionT {
    Submit,
    Store,
    Publish,
}

#[derive(Debug)]
struct Action {
    args: Vec<serde_json::Value>,
    doc: serde_json::Map<String, serde_json::Value>,
    act: ActionT,
}

#[derive(Debug)]
enum Error {
    UnknownAction,
    InvalidAction,
    Protocol,
    Json {
        err: serde_json::Error
    },
}

fn do_submit(
    args: &Vec<serde_json::Value>,
    d: &serde_json::Map<String, serde_json::Value>
) -> bool {
    debug!("submit: {:?}, {:?}", args, d);
    true
}

fn do_store(
    args: &Vec<serde_json::Value>,
    d: &serde_json::Map<String, serde_json::Value>
) -> bool {
    debug!("store: {:?}, {:?}", args, d);
    true
}

fn do_publish(
    args: &Vec<serde_json::Value>,
    d: &serde_json::Map<String, serde_json::Value>
) -> bool {
    debug!("publish: {:?}, {:?}", args, d);
    true
}

fn process_cmd(
    cmd_v: &serde_json::Value,
    args_v: &serde_json::Value,
    doc_v: &serde_json::Value
) -> Result<Action, Error> {
    match (cmd_v, args_v, doc_v) {
        (serde_json::Value::String(cmd), serde_json::Value::Array(args), serde_json::Value::Object(d)) => {
            match cmd.as_str() {
                "SUBMIT" => Ok(Action { args: args.clone(), doc: d.clone(), act: ActionT::Submit }),
                "STORE" => Ok(Action { args: args.clone(), doc: d.clone(), act: ActionT::Store }),
                "PUBLISH" => Ok(Action { args: args.clone(), doc: d.clone(), act: ActionT::Publish }),
                _ => {
                    Err(Error::UnknownAction)
                }
            }
        },
        _ => {
            Err(Error::InvalidAction)
        }
    }
}

fn process_text(t: String) -> Result<Action, Error> {
    info!("text: {:?}", t);
    let v: serde_json::Result<serde_json::Value> = serde_json::from_str(t.as_str());
    match v {
        Ok(serde_json::Value::Array(a)) => {
            match a.len() {
                3 => {
                    process_cmd(&a[0], &a[1], &a[2])
                },
                _ => {
                    Err(Error::Protocol)
                }
            }
        },
        Ok(_) => {
            debug!("valid but not interested");
            Err(Error::Protocol)
        },
        Err(err) => {
            Err(Error::Json { err: err })
        }
    }
}

fn process(tx: tokio::sync::mpsc::Sender<Result<Action, Error>>, msg: Option<Result<Message, tungstenite::Error>>) -> bool {
    match msg {
        Some(Ok(Message::Text(t))) => {
            let proc_res = process_text(t);
            debug!("proc_res: {:?}", proc_res);
            match proc_res {
                Ok(Action { args, doc, act }) => {
                    tokio::spawn(async move {
                        match act {
                            ActionT::Submit => { do_submit(&args, &doc) },
                            ActionT::Store => { do_store(&args, &doc) },
                            ActionT::Publish => { do_publish(&args, &doc) },
                        }
                    });
                },
                Err(err) => {
                    debug!("action err: {:?}", err);
                }
            }
            true
        },
        Some(Ok(Message::Binary(_))) => {
            info!("binary");
            true
        },
        Some(Ok(Message::Ping(_) | Message::Pong(_))) => {
            info!("ping/pong");
            true
        }
        Some(Err(TTError::Capacity(MessageTooLong{size: _, max_size: _}))) => {
            info!("size");
            true
        },
        None |
        Some(Ok(Message::Close(_)) |
             Err(TTError::AlreadyClosed | TTError::ConnectionClosed |
                 TTError::Protocol(tungstenite::error::ProtocolError::ResetWithoutClosingHandshake)))
            => {
                info!("close or error");
                false
            },
        Some(Err(TTError::Io(e))) => {
            // IO errors are considered fatal
            warn!("io error: {:?}", e);
            false
        }
        x => {
            // default condition on error is to close the client connection
            info!("unknown: {:?}", x);
            false
        }
    }
}

async fn handle_connection(peer: SocketAddr, stream: TcpStream) -> TTResult<()> {
    let ws_stream = accept_async(stream).await.expect("Failed to accept");
    info!("New WebSocket connection: {}", peer);
    let (mut _ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = mpsc::channel(64);

    loop {
        tokio::select! {
            msg = ws_receiver.next() => {
                match process(tx.clone(), msg) {
                    true => continue,
                    false => break,
                }
            }
            Some(resp) = rx.recv() => {
                debug!("resp: {:?}", resp);
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
