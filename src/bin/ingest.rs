use log::*;
use rusqlite::Connection;
use glob::glob;
use std::{
    env,
    path::PathBuf,
};

fn open_db() -> Connection {
    let should_init = !std::path::Path::new("data/rules.db").exists();

    if !std::path::Path::new("data").exists() {
        std::fs::create_dir("./data").expect("failed to create directory");
    }

    let conn = Connection::open("data/rules.db").unwrap();

    if should_init {
        conn.execute_batch(
          "BEGIN;
           CREATE TABLE IF NOT EXISTS in_effect (
                 id           INTEGER PRIMARY KEY AUTOINCREMENT,
                 rule_id      text,
                 version      text,
                 jurisdiction text,
                 from_t       text,
                 to_t         text,
                 tz           text
           );
           CREATE TABLE IF NOT EXISTS applicable (
                 id      int  PRIMARY KEY,
                 rule_id text,
                 version text,
                 key     text
           );
           COMMIT;").unwrap();
    }

    conn
}

#[derive(Debug, Clone)]
struct InEffect {
    pub loc: String,
    pub from: String,
    pub to: String,
    pub tz: String,
}

fn store_in_effect(conn: &Connection, id: &String, rev: u64, in_effect: &Vec<InEffect>) {
    let mut stmt = conn.prepare("
      INSERT INTO in_effect (rule_id, version, jurisdiction, from_t, to_t, tz)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)").unwrap();
    for ie in in_effect.iter() {
        stmt.execute([
            id.clone(),
            rev.to_string(),
            ie.loc.clone(),
            ie.from.clone(),
            ie.to.clone(),
            ie.tz.clone(),
        ]).unwrap();

        debug!("store: {:?} = {:?}", id, ie);
    }
}

fn make_in_effect(o: &serde_json::Value) -> InEffect {
    InEffect {
        loc: o["in"].as_str().unwrap().to_string(),
        from: o["from"].as_str().unwrap().to_string(),
        to: o["to"].as_str().unwrap().to_string(),
        tz: o["tz"].as_str().unwrap().to_string(),
    }
}

pub fn store_keys(conn: &Connection, id: &String, rev: u64, keys: &Vec<String>) -> bool {
    let mut stmt = conn.prepare("INSERT INTO applicable (rule_id, version, key) VALUES (?1, ?2, ?3)").unwrap();
    for k in keys.iter() {
        stmt.execute([id.clone(), rev.to_string(), k.clone()]).unwrap();
    }
    true
}

fn rule_dir(id: &String) -> PathBuf {
    [".", "data", "rules", id].iter().collect()
}

fn extract_rev(p: &PathBuf) -> u64 {
    match p.as_path().file_stem() {
        None => 0,
        Some(st) => { st.to_str().unwrap().parse().unwrap() }
    }
}

fn find_latest_rule(id: &String) -> Option<u64> {
    let dir = rule_dir(id);
    let vers = dir.join("*.json");

    println!("searching for rules: vers={:?}", vers);
    let latest = match glob(vers.to_str().unwrap()) {
        Ok(it) => it.filter_map(|p| p.ok()).max_by_key(extract_rev),
        _ => None
    };

    match latest {
        Some(p) => Some(extract_rev(&p)),
        None => None
    }
}

// follows an update-or-insert model: if the rule has an 'id' property,
// that's used to update/insert the rule. Otherwise, it's assumed the rule is new.
fn main() {
    let args: Vec<String> = env::args().collect();
    match args.as_slice() {
        [_, path] => {
            println!("path={:?}", path);
            let f = std::fs::File::open(&path).expect("could not open file");
            let o: serde_json::Value = serde_json::from_reader(f).expect("parse failure");

            let id = match &o["properties"]["id"] {
                serde_json::Value::String(s) => {
                    s.to_string()
                },
                _ => {
                    uuid::Uuid::new_v4().hyphenated().to_string()
                }
            };

            let conn = open_db();
            println!("id={:?}", id);

            let rev = match find_latest_rule(&id) {
                Some(r) => r + 1,
                None => 0
            };
            let path = rule_dir(&id).join(format!("{:?}.json", rev));
            println!("rev={:?}; path={:?}", rev, path);

            // TODO: design a binary format to retain, not JSON, which we need to parse
            match std::fs::File::create(&path) {
                Ok(f) => {
                    match serde_json::to_writer(f, &o) {
                        Ok(_) => {
                            println!("wrote rule (rule={:?}", path);
                        },
                        Err(e) => {
                            println!("failed to write rule (rule={:?}; e={:?})", path, e);
                        }
                    }
                },
                Err(e) => {
                    println!("failed to create file (rule={:?}; e={:?}", path, e);
                }
            }

            match &o["in_effect"] {
                serde_json::Value::Array(ie) => {
                    let vals: Vec<InEffect> = ie.iter().map(make_in_effect).collect();
                    store_in_effect(&conn, &id, rev, &vals);
                },
                _ => {
                    println!("no in effect");
                }
            }
            match &o["input_conditions"] {
                serde_json::Value::Array(conds) => {
                    let keys: Vec<String> = conds.iter().map(|v| match v {
                        serde_json::Value::Object(m) => {
                            debug!("map: {:?}", m);
                            Some(m["expression"]["key"].as_str().unwrap().to_string())
                        },
                        _ => None
                    }).flatten().collect();

                    debug!("keys: {:?}", keys);
                    store_keys(&conn, &id, rev, &keys);
                },
                _ => {
                    debug!("no conditions");
                }
            }
        },
        _ => {
            println!("invalid args");
        }
    }
}
