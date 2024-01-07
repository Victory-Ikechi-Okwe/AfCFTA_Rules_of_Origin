use log::*;
use rusqlite::{Connection};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct InEffect {
    pub loc: String,
    pub from: String,
    pub to: String,
    pub tz: String,
}

pub fn store(id: String, effects: &Vec<InEffect>) -> bool {
//    let conn = Connection::open("data/rules.db")?;
    let conn = Connection::open("data/rules.db").unwrap();

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

    let mut stmt = conn.prepare("
      INSERT INTO in_effect (rule_id, version, jurisdiction, from_t, to_t, tz)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)").unwrap();
    for ie in effects.iter() {
        stmt.execute([
            id.clone(),
            String::from(""),
            ie.loc.clone(),
            ie.from.clone(),
            ie.to.clone(),
            ie.tz.clone(),
        ]).unwrap();

        debug!("store: {:?} = {:?}", id, ie);
    }
    true
}
