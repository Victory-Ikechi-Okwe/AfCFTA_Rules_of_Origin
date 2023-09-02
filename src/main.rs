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

use std::path::PathBuf;
use glob::glob;

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
    Get,
    Publish,
    Store,
    Submit,
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

fn extract_rev(p: &PathBuf) -> i32 {
    match p.as_path().file_stem() {
        None => -9999,
        Some(st) => { st.to_str().unwrap().parse().unwrap() }
    }
}

fn find_latest_rule(path: &PathBuf) -> Option<PathBuf> {
    let vers = path.join("*.json");

    debug!("searching for rules: vers={:?}", vers);
    match glob(vers.to_str().unwrap()) {
        Ok(it) => it.filter_map(|p| p.ok()).max_by_key(extract_rev),
        _ => None
    }
}

fn next_stored_rule(path: &PathBuf) -> PathBuf {
    match find_latest_rule(path) {
        Some(latest_path) => {
            debug!("found latest (latest_path={:?})", latest_path);
            match latest_path.as_path().file_stem() {
                Some(st) => {
                    let rev: i32 = st.to_str().unwrap().parse().unwrap();
                    debug!("next rev (rev={:?})", rev);
                    path.join(format!("{:?}.json", rev + 1))
                },
                None => path.join("1.json")
            }
        },
        None => path.join("1.json")
    }
}

fn rule_path(id: &String) -> PathBuf {
    [".", "data", "rules", &id].iter().collect()
}

fn assure_dir_for_rule(id: &String) -> PathBuf {
    let path: PathBuf = rule_path(id);

    match std::fs::create_dir_all(&path) {
        Err(e) => debug!("failed to create store dir (dir={:?}, e={:?}", path, e),
        _ => { }
    };

    path
}

fn store_rule(ofn: PathBuf, d: &serde_json::Map<String, serde_json::Value>) -> bool {
    debug!("writing rule (ofn={:?})", ofn);
    match std::fs::File::create(&ofn) {
        Ok(f) => {
            match serde_json::to_writer(f, &d) {
                Ok(_) => {
                    debug!("wrote rule (ofn={:?}", ofn);
                    true
                },
                Err(e) => {
                    debug!("failed to write rule (ofn={:?}; e={:?})", ofn, e);
                    false
                }
            }
        },
        Err(e) => {
            debug!("failed to create file (ofn={:?}; e={:?}", ofn, e);
            false
        }
    }
}

fn find_rule_by_rev(id: &String, rev: i64) -> Option<PathBuf> {
    let path = rule_path(id).join(format!("{:?}.json", rev));
    if path.exists() { Some(path) } else { None }
}

fn find_rule_by_args(args: &Vec<serde_json::Value>) -> Option<PathBuf> {
    match args.as_slice() {
        [serde_json::Value::String(id), serde_json::Value::Number(rev), ..] => {
            debug!("id={:?}; rev={:?}", id, rev);
            find_rule_by_rev(id, rev.as_i64().unwrap())
        },
        [serde_json::Value::String(id)] => {
            find_latest_rule(&rule_path(&id.to_string()))
        },
        _ => None
    }
}

// [id, (rev)]
// publish doc[id], optional (rev) to publish
fn do_publish(
    args: &Vec<serde_json::Value>,
    d: &serde_json::Map<String, serde_json::Value>
) -> bool {
    debug!("publish: {:?}, {:?}", args, d);

    match find_rule_by_args(args) {
        Some(path) => {
            debug!("located rule (path={:?})", path);
            let target = path.parent().unwrap().join("published");
            let _ = std::fs::remove_file(&target);
            match std::os::unix::fs::symlink(&path, &target) {
                Ok(_) => {
                    debug!("linked (path={:?}, target={:?}", path, target);
                    true
                },
                _ => {
                    debug!("failed link (path={:?}, target={:?}", path, target);
                    true
                }
            }
        },
        None => {
            debug!("rule not found");

            false
        }
    }
}

// [(id)], { to_store } -> [id, rev]
// store document, id? new rev : new doc
fn do_store(
    args: &Vec<serde_json::Value>,
    d: &serde_json::Map<String, serde_json::Value>
) -> bool {
    debug!("store: {:?}, {:?}", args, d);

    let id_opt = match args.as_slice() {
        [serde_json::Value::String(id), ..] => {
            Some(id.to_string())
        },
        [] => {
            Some(uuid::Uuid::new_v4().hyphenated().to_string())
        },
        _ => None
    };

    match id_opt {
        Some(id) => {
            let path = assure_dir_for_rule(&id);
            let ofn = next_stored_rule(&path);

            debug!("storing rule (path={:?}; ofn={:?}", path, ofn);
            store_rule(ofn, d);

            true
        },
        None => {
            debug!("store: no id");
            false
        }
    }
}

fn do_get(
    args: &Vec<serde_json::Value>,
    d: &serde_json::Map<String, serde_json::Value>
) -> bool {
    debug!("get: {:?}, {:?}", args, d);
    match find_rule_by_args(args) {
        Some(path) => {
            debug!("GET: found rule (path={:?}; args={:?})", path, args);
            true
        },
        None => {
            debug!("GET: rule not found (args={:?})", args);
            true
        }
    }
}

fn do_submit(
    args: &Vec<serde_json::Value>,
    d: &serde_json::Map<String, serde_json::Value>
) -> bool {
    debug!("submit: {:?}, {:?}", args, d);
    true
}

fn process_cmd(
    cmd: &String,
    args: &Vec<serde_json::Value>,
    doc: &serde_json::Map<String, serde_json::Value>
) -> Result<Action, Error> {
    match cmd.as_str() {
        "GET"     => Ok(Action { args: args.clone(), doc: doc.clone(), act: ActionT::Get }),
        "PUBLISH" => Ok(Action { args: args.clone(), doc: doc.clone(), act: ActionT::Publish }),
        "STORE"   => Ok(Action { args: args.clone(), doc: doc.clone(), act: ActionT::Store }),
        "SUBMIT"  => Ok(Action { args: args.clone(), doc: doc.clone(), act: ActionT::Submit }),
        _ => {
            Err(Error::UnknownAction)
        }
    }
}

fn process_text(t: String) -> Result<Action, Error> {
    info!("text: {:?}", t);
    let v: serde_json::Result<serde_json::Value> = serde_json::from_str(t.as_str());
    match v {
        Ok(serde_json::Value::Array(a)) => {
            match a.as_slice() {
                [serde_json::Value::String(cmd), serde_json::Value::Array(args), serde_json::Value::Object(d)] => {
                    process_cmd(cmd, args, d)
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
                            ActionT::Get     => { do_get(&args, &doc) },
                            ActionT::Publish => { do_publish(&args, &doc) },
                            ActionT::Store   => { do_store(&args, &doc) },
                            ActionT::Submit  => { do_submit(&args, &doc) },
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
