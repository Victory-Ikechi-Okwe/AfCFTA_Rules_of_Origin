use log::*;
use rusqlite::{Connection};

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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS in_effect (
              id           INTEGER PRIMARY KEY AUTOINCREMENT,
              rule_id      text,
              version      text,
              jurisdiction text,
              from_t       text,
              to_t         text,
              tz           text
         )", []).unwrap();
    for ie in effects.iter() {
        debug!("store: {:?} = {:?}", id, ie);
    }
    true
}
