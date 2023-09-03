use futures_util::{StreamExt, SinkExt};
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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

type Peers = Arc<Mutex<HashMap<SocketAddr, AtomicU64>>>;

async fn accept_connection(peer: SocketAddr, stream: TcpStream, peers: Peers) {
    if let Err(e) = handle_connection(peer, stream, &peers).await {
        match e {
            TTError::ConnectionClosed | TTError::Protocol(_) | TTError::Utf8 => (),
            err => error!("Error processing connection: {}", err),
        }
    }
}


#[derive(Debug, Copy, Clone)]
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

#[derive(Debug, Copy, Clone)]
enum ReactionStatus {
    Ok,
    Failed,
    Unknown,
}

#[derive(Debug)]
struct Reaction {
    status: ReactionStatus,
    msg: String,
    doc: Option<serde_json::Value>,
    act: ActionT,
    order: u64,
}

fn make_failed(order: u64, act: ActionT, msg: &str) -> Reaction {
    Reaction { status: ReactionStatus::Failed, msg: msg.to_string(), doc: None, act: act, order: order }
}

fn make_failed_with_string(order: u64, act: ActionT, msg: String) -> Reaction {
    Reaction { status: ReactionStatus::Failed, msg: msg.clone(), doc: None, act: act, order: order }
}

fn make_ok(order: u64, act: ActionT, msg: &str) -> Reaction {
    Reaction { status: ReactionStatus::Ok, msg: msg.to_string(), doc: None, act: act, order: order }
}

fn make_ok_with_json(order: u64, act: ActionT, doc: &serde_json::Value) -> Reaction {
    Reaction { status: ReactionStatus::Ok, msg: String::new(), order: order, act: act, doc: Some(doc.clone()) }
}

fn make_action_string(act: &ActionT) -> String {
    match act {
        ActionT::Get => String::from("get"),
        ActionT::Publish => String::from("publish"),
        ActionT::Store => String::from("store"),
        ActionT::Submit => String::from("submit"),
    }
}

fn make_status_string(st: &ReactionStatus) -> String {
    match st {
        ReactionStatus::Ok => String::from("ok"),
        ReactionStatus::Failed => String::from("failed"),
        ReactionStatus::Unknown => String::from("unknown"),
    }
}

fn make_reaction_message(r: &Reaction) -> Message {
    let v = match &r.doc {
        Some(doc) => serde_json::json!([
            r.order,
            make_status_string(&r.status),
            make_action_string(&r.act),
            r.doc]),
        None => serde_json::json!([
            r.order,
            make_status_string(&r.status),
            make_action_string(&r.act),
            r.msg]),
    };

    debug!("json: {:?}", v);

    Message::Text(v.to_string())
}

fn make_accepted_message(order: u64) -> Message {
    let v = serde_json::json!([order, "accepted"]);
    Message::Text(v.to_string())
}

fn make_rejected_message(order: u64) -> Message {
    let v = serde_json::json!([order, "rejected"]);
    Message::Text(v.to_string())
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
    d: &serde_json::Map<String, serde_json::Value>,
    order: u64
) -> Reaction {
    debug!("publish: {:?}, {:?}", args, d);

    match find_rule_by_args(args) {
        Some(path) => {
            debug!("located rule (path={:?})", path);
            let target = path.parent().unwrap().join("published");
            let _ = std::fs::remove_file(&target);
            match std::os::unix::fs::symlink(&path, &target) {
                Ok(_) => {
                    debug!("linked (path={:?}, target={:?}", path, target);
                    make_ok(order, ActionT::Publish, "")
                },
                _ => {
                    debug!("failed link (path={:?}, target={:?}", path, target);
                    make_failed(order, ActionT::Publish, "link failed")
                }
            }
        },
        None => {
            debug!("rule not found");
            make_failed(order, ActionT::Publish, "rule not found")
        }
    }
}

// [(id)], { to_store } -> [id, rev]
// store document, id? new rev : new doc
fn do_store(
    args: &Vec<serde_json::Value>,
    d: &serde_json::Map<String, serde_json::Value>,
    order: u64
) -> Reaction {
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

            make_ok(order, ActionT::Store, "stored")
        },
        None => {
            debug!("store: no id");
            make_failed(order, ActionT::Store, "unknown id")
        }
    }
}

fn do_get(
    args: &Vec<serde_json::Value>,
    d: &serde_json::Map<String, serde_json::Value>,
    order: u64
) -> Reaction {
    debug!("get: {:?}, {:?}", args, d);
    match find_rule_by_args(args) {
        Some(path) => {
            debug!("GET: found rule (path={:?}; args={:?})", path, args);
            let f = match std::fs::File::open(&path) {
                Ok(f) => f,
                _ => {
                    return make_failed_with_string(
                        order, ActionT::Get, format!("failed to open rule file (path={:?})", path)
                    );
                }
            };

            make_ok_with_json(order, ActionT::Get, &serde_json::from_reader(f).unwrap())
        },
        None => {
            debug!("GET: rule not found (args={:?})", args);
            make_failed(order, ActionT::Get, "rule not found")
        }
    }
}

    // status: ReactionStatus,
    // msg: String,
    // doc: serde_json::Map<String, serde_json::Value>,
    // act: ActionT,
    // order: u64,

fn do_submit(
    args: &Vec<serde_json::Value>,
    d: &serde_json::Map<String, serde_json::Value>,
    order: u64
) -> Reaction {
    debug!("submit: {:?}, {:?}", args, d);
    make_ok(order, ActionT::Submit, "")
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

fn process(
    tx: tokio::sync::mpsc::Sender<Reaction>,
    order: u64,
    msg: Option<Result<Message, tungstenite::Error>>
) -> bool {
    match msg {
        Some(Ok(Message::Text(t))) => {
            let action = process_text(t);
            debug!("parsed arction (action={:?})", action);
            match action {
                Ok(Action { args, doc, act }) => {
                    tokio::spawn(async move {
                        // TODO: differential the JSON written - this is just telling the peer
                        // that the message is correct and so they can learn the order -
                        // maybe Reaction isn't the correct thing to transmit? maybe there should
                        // be a wrapper - esp considering the unimplemented error below?
                        // order would be in that wrapper
                        // Wrapper { reaction: reaction, order: order }??
                        let reaction = match act {
                            ActionT::Get     => { do_get(&args, &doc, order) },
                            ActionT::Publish => { do_publish(&args, &doc, order) },
                            ActionT::Store   => { do_store(&args, &doc, order) },
                            ActionT::Submit  => { do_submit(&args, &doc, order) },
                        };
                        debug!("reaction (reaction={:?})", reaction);
                        let _b = tx.send(reaction).await;
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

fn update_order(peers: &Peers, peer: SocketAddr) -> u64 {
    let m = peers.lock().unwrap();
    match m.get(&peer) {
        Some(a) => a.fetch_add(1, Ordering::SeqCst),
        None => 0,
    }
}

async fn handle_connection(peer: SocketAddr, stream: TcpStream, peers: &Peers) -> TTResult<()> {
    let ws_stream = accept_async(stream).await.expect("Failed to accept");
    info!("New WebSocket connection: {}", peer);
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (tx, mut rx) = mpsc::channel(64);

    peers.lock().unwrap().insert(peer, AtomicU64::new(0));

    loop {
        tokio::select! {
            msg = ws_receiver.next() => {
                let order = update_order(peers, peer);
                debug!("order: {:?}", order);
                if process(tx.clone(), order, msg) {
                    ws_sender.send(make_accepted_message(order)).await;
                    continue;
                } else {
                    ws_sender.send(make_rejected_message(order)).await;
                    break;
                }
            }
            Some(resp) = rx.recv() => {
                debug!("resp: {:?}", resp);
                ws_sender.send(make_reaction_message(&resp)).await?;
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let addr = "127.0.0.1:9002";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    let peers = Peers::new(Mutex::new(HashMap::new()));

    info!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream.peer_addr().expect("connected streams should have a peer address");
        info!("Peer address: {}", peer);

        tokio::spawn(accept_connection(peer, stream, peers.clone()));
    }
}
