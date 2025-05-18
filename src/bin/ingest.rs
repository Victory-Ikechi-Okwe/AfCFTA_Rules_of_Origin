use chrono::{ DateTime, Utc };
use chrono_tz::Tz;
use log::*;
use rusqlite::Connection;
use glob::glob;
use std::{
    io::{ self },
    path::PathBuf,
};

use rookie::rules::{
    InEffect,
    Rule,
    parser,
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

fn store_in_effect(conn: &Connection, id: &String, rev: u64, in_effect: &Vec<InEffect>) {
    let mut stmt = conn.prepare("
      INSERT INTO in_effect (rule_id, version, jurisdiction, from_t, to_t, tz)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)").unwrap();
    for ie in in_effect.iter() {
        stmt.execute([
            id.clone(),
            rev.to_string(),
            ie.jurisdiction.clone().unwrap_or_else(|| String::from("")).clone(),
            ie.from.unwrap_or_else(|| DateTime::<Utc>::default()).to_string(),
            ie.to.unwrap_or_else(|| DateTime::<Utc>::default()).to_string(),
            ie.tz.unwrap_or_else(|| Tz::default()).to_string(),
        ]).unwrap();

        debug!("store: {:?} = {:?}", id, ie);
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

fn find_latest_rev(id: &String) -> Option<u64> {
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

fn build_applicable(vals: &serde_json::Value) -> Option<Vec<String>> {
    match vals {
        serde_json::Value::Array(conds) => {
            let keys: Vec<String> = conds.iter().map(|v| match v {
                serde_json::Value::Object(m) => {
                    debug!("map: {:?}", m);
                    Some(m["expression"]["key"].as_str().unwrap().to_string())
                },
                _ => None
            }).flatten().collect();

            Some(keys)
        },
        _ => None
    }
}

fn store_rule(id: &String, rev: u64, rule_fn: &String) {
    let path = rule_dir(&id).join(format!("{:?}.json", rev));
    println!("rev={:?}; path={:?}", rev, path);

    println!("copy: fr={}; to={:?}", rule_fn, path);
    std::fs::create_dir_all(path.parent().unwrap());
    std::fs::copy(rule_fn, path);
}

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    rule_fn: String,
}

// follows an update-or-insert model: if the rule has an 'id' property,
// that's used to update/insert the rule. Otherwise, it's assumed the rule is new.
fn main()  -> io::Result<()>{
    let args = Args::parse();

    println!("rules_fn={:?}", args.rule_fn);
    let mut prsr = parser::Parse::new();
    prsr.parse_file(&args.rule_fn)?;

    let rule = &prsr.rule;
    let id = rule.id();

    let rev = match find_latest_rev(&id) {
        Some(r) => r + 1,
        None => 0
    };

    println!("id={}; rev={}", id, rev);

    let conn = open_db();

    store_in_effect(&conn, &id, rev, &rule.in_effect);

    let keys: Vec<_> = rule.conditions.iter().map(|c| c.key.clone()).collect();
    println!("keys={:?}", keys);

    store_keys(&conn, &id, rev, &keys);

    store_rule(&id, rev, &args.rule_fn);

    Ok(())
}
