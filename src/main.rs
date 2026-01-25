use futures_util::{SinkExt, StreamExt};
use log::*;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Error as TTError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::Result as TTResult;
use tungstenite::error::CapacityError::MessageTooLong;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

mod db;
mod rule;
mod submissions;

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
    Json { err: serde_json::Error },
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
    doc: serde_json::Value,
    act: ActionT,
    order: u64,
}

fn make_failed(order: u64, act: ActionT, m: String) -> Reaction {
    let doc = serde_json::json!({ "error" : m });
    Reaction {
        status: ReactionStatus::Failed,
        doc: doc,
        act: act,
        order: order,
    }
}

fn make_failed_with_str(order: u64, act: ActionT, m: &str) -> Reaction {
    make_failed(order, act, m.to_string())
}

fn make_ok(order: u64, act: ActionT, doc: &serde_json::Value) -> Reaction {
    Reaction {
        status: ReactionStatus::Ok,
        order: order,
        act: act,
        doc: doc.clone(),
    }
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
    let doc = serde_json::json!([
        r.order,
        make_status_string(&r.status),
        make_action_string(&r.act),
        r.doc
    ]);
    debug!("json: {:?}", doc);

    Message::Text(doc.to_string().into())
}

fn make_accepted_message(order: u64) -> Message {
    let v = serde_json::json!([order, "accepted"]);
    Message::Text(v.to_string().into())
}

fn make_rejected_message(order: u64) -> Message {
    let v = serde_json::json!([order, "rejected"]);
    Message::Text(v.to_string().into())
}

fn find_rule_by_args(args: &Vec<serde_json::Value>) -> Option<rule::Rule> {
    match args.as_slice() {
        [serde_json::Value::String(id), serde_json::Value::Number(rev), ..] => {
            debug!("id={:?}; rev={:?}", id, rev);
            rule::find_rule_by_rev(id, rev.as_u64().unwrap())
        }
        [serde_json::Value::String(id)] => {
            debug!("id={:?}; published", id);
            rule::find_published_rule(id)
        }
        _ => None,
    }
}

// [id, (rev)]
// publish doc[id], optional (rev) to publish
fn do_publish(
    args: &Vec<serde_json::Value>,
    d: &serde_json::Map<String, serde_json::Value>,
    order: u64,
) -> Reaction {
    debug!("publish: {:?}, {:?}", args, d);

    match find_rule_by_args(args) {
        Some(rule) => {
            if rule.publish() {
                let doc = serde_json::json!({ "id" : rule.id.clone(), "revision" : rule.rev });
                make_ok(order, ActionT::Publish, &doc)
            } else {
                make_failed_with_str(order, ActionT::Publish, "failed change version")
            }
        }
        None => {
            debug!("rule not found");
            make_failed_with_str(order, ActionT::Publish, "rule not found")
        }
    }
}

fn val_to_in_effect(o: &serde_json::Value) -> db::InEffect {
    db::InEffect {
        loc: o["in"].as_str().unwrap().to_string(),
        from: o["from"].as_str().unwrap().to_string(),
        to: o["to"].as_str().unwrap().to_string(),
        tz: o["tz"].as_str().unwrap().to_string(),
    }
}

// [(id)], { to_store } -> [id, rev]
// store document, id? new rev : new doc
fn do_store(
    args: &Vec<serde_json::Value>,
    d: &serde_json::Map<String, serde_json::Value>,
    order: u64,
) -> Reaction {
    debug!("store: {:?}, {:?}", args, d);

    let id_opt = match args.as_slice() {
        [serde_json::Value::String(id), ..] => Some(id.to_string()),
        [] => Some(uuid::Uuid::new_v4().hyphenated().to_string()),
        _ => None,
    };

    match id_opt {
        Some(id) => {
            let rule = rule::next_revision(&id);
            let resp = serde_json::json!({ "id" : rule.id, "revision" : rule.rev });

            if rule.store(&d) {
                match &d["in_effect"] {
                    serde_json::Value::Array(ie) => {
                        let vals: Vec<db::InEffect> = ie.iter().map(val_to_in_effect).collect();
                        db::store(&id, &vals);
                    }
                    _ => {
                        debug!("no in_effect");
                    }
                }

                match &d["input_conditions"] {
                    serde_json::Value::Array(conds) => {
                        let keys: Vec<String> = conds
                            .iter()
                            .map(|v| match v {
                                serde_json::Value::Object(m) => {
                                    debug!("map: {:?}", m);
                                    Some(m["expression"]["key"].as_str().unwrap().to_string())
                                }
                                _ => None,
                            })
                            .flatten()
                            .collect();

                        debug!("keys: {:?}", keys);
                        db::store_keys(&id, &keys);
                    }
                    _ => {
                        debug!("no conditions");
                    }
                }

                make_ok(order, ActionT::Store, &resp)
            } else {
                make_failed_with_str(order, ActionT::Publish, "failed store")
            }
        }
        None => {
            debug!("store: no id");
            make_failed_with_str(order, ActionT::Store, "unknown id")
        }
    }
}

fn do_get(
    args: &Vec<serde_json::Value>,
    d: &serde_json::Map<String, serde_json::Value>,
    order: u64,
) -> Reaction {
    debug!("get: {:?}, {:?}", args, d);
    match find_rule_by_args(args) {
        Some(rule) => {
            debug!("GET: found rule (rule={:?}; args={:?})", rule, args);
            let f = match std::fs::File::open(&rule.path()) {
                Ok(f) => f,
                _ => {
                    return make_failed(
                        order,
                        ActionT::Get,
                        format!("failed to open rule file (path={:?})", rule.path()),
                    );
                }
            };

            make_ok(order, ActionT::Get, &serde_json::from_reader(f).unwrap())
        }
        None => {
            debug!("GET: rule not found (args={:?})", args);
            make_failed_with_str(order, ActionT::Get, "rule not found")
        }
    }
}

fn do_submit(
    args: &Vec<serde_json::Value>,
    d: &serde_json::Map<String, serde_json::Value>,
    order: u64,
) -> Reaction {
    debug!("submit: {:?}, {:?}", args, d);

    match submissions::store(d) {
        Some(id) => {
            let resp = serde_json::json!({ "id" : id });
            make_ok(order, ActionT::Submit, &resp)
        }
        None => make_failed_with_str(order, ActionT::Publish, "failed to queue"),
    }
}

fn process_cmd(
    cmd: &String,
    args: &Vec<serde_json::Value>,
    doc: &serde_json::Map<String, serde_json::Value>,
) -> Result<Action, Error> {
    match cmd.as_str() {
        "GET" => Ok(Action {
            args: args.clone(),
            doc: doc.clone(),
            act: ActionT::Get,
        }),
        "PUBLISH" => Ok(Action {
            args: args.clone(),
            doc: doc.clone(),
            act: ActionT::Publish,
        }),
        "STORE" => Ok(Action {
            args: args.clone(),
            doc: doc.clone(),
            act: ActionT::Store,
        }),
        "SUBMIT" => Ok(Action {
            args: args.clone(),
            doc: doc.clone(),
            act: ActionT::Submit,
        }),
        _ => Err(Error::UnknownAction),
    }
}

fn process_text(t: &str) -> Result<Action, Error> {
    info!("text: {:?}", t);
    let v: serde_json::Result<serde_json::Value> = serde_json::from_str(t);
    match v {
        Ok(serde_json::Value::Array(a)) => match a.as_slice() {
            [serde_json::Value::String(cmd), serde_json::Value::Array(args), serde_json::Value::Object(d)] => {
                process_cmd(cmd, args, d)
            }
            _ => Err(Error::Protocol),
        },
        Ok(_) => {
            debug!("valid but not interested");
            Err(Error::Protocol)
        }
        Err(err) => Err(Error::Json { err: err }),
    }
}

fn process(
    tx: tokio::sync::mpsc::Sender<Reaction>,
    order: u64,
    msg: Option<Result<Message, tungstenite::Error>>,
) -> bool {
    match msg {
        Some(Ok(Message::Text(t))) => {
            let action = process_text(&t);
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
                            ActionT::Get => do_get(&args, &doc, order),
                            ActionT::Publish => do_publish(&args, &doc, order),
                            ActionT::Store => do_store(&args, &doc, order),
                            ActionT::Submit => do_submit(&args, &doc, order),
                        };
                        debug!("reaction (reaction={:?})", reaction);
                        let _b = tx.send(reaction).await;
                    });
                }
                Err(err) => {
                    debug!("action err: {:?}", err);
                }
            }
            true
        }
        Some(Ok(Message::Binary(_))) => {
            info!("binary");
            true
        }
        Some(Ok(Message::Ping(_) | Message::Pong(_))) => {
            info!("ping/pong");
            true
        }
        Some(Err(TTError::Capacity(MessageTooLong {
            size: _,
            max_size: _,
        }))) => {
            info!("size");
            true
        }
        None
        | Some(
            Ok(Message::Close(_))
            | Err(
                TTError::AlreadyClosed
                | TTError::ConnectionClosed
                | TTError::Protocol(tungstenite::error::ProtocolError::ResetWithoutClosingHandshake),
            ),
        ) => {
            info!("close or error");
            false
        }
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
                    let _ = ws_sender.send(make_accepted_message(order)).await;
                    continue;
                } else {
                    let _ = ws_sender.send(make_rejected_message(order)).await;
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
        let peer = stream
            .peer_addr()
            .expect("connected streams should have a peer address");
        info!("Peer address: {}", peer);

        tokio::spawn(accept_connection(peer, stream, peers.clone()));
    }
}
