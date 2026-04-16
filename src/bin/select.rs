use clap::Parser;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
use rookie::rules::{eval_conds, print_results, parser, parser::RulesetParser};

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
           COMMIT;",
        )
        .unwrap();
    }
    conn
}

fn doc_keys(doc: &serde_json::Value) -> Vec<String> {
    keys(doc, true)
}

fn keys(doc: &serde_json::Value, at_root: bool) -> Vec<String> {
    match doc {
        serde_json::Value::Object(o) => {
            let mut v = vec![];
            for (k, val) in o {
                v.push(k.clone());
                v.extend(keys(val, false).iter().map(|s| format!("{}{}", k, s)));
            }
            if at_root {
                v
            } else {
                v.iter().map(|s| format!(".{}", s)).collect()
            }
        }
        serde_json::Value::Array(a) => {
            let mut v = vec![];
            for (i, it) in a.iter().enumerate() {
                v.push(format!("[{}]", i));
                v.extend(keys(it, false).iter().map(|s| format!("[{}]{}", i, s)));
            }
            v
        }
        _ => vec![],
    }
}

#[derive(serde::Deserialize, Debug)]
struct Context {
    jurisdiction: String,
    tz: String,
}

fn load_context() -> Option<Context> {
    let f = std::fs::File::open("etc/contexts/default.json").expect("error opening context");
    serde_json::from_reader(f).ok()
}

#[derive(Debug, Clone)]
struct Ref {
    id: String,
    version: String,
}

fn find(ctx: Context, conn: Connection, keys: Vec<String>) -> rusqlite::Result<Vec<Ref>> {
    let markers = ["?"].repeat(keys.len()).join(",");
    let q = format!(
        "SELECT DISTINCT e.rule_id, e.version
         FROM in_effect AS e
         JOIN applicable AS a ON e.rule_id=a.rule_id AND e.version=a.version
         WHERE e.jurisdiction=? AND e.tz=? AND a.key IN ({})",
        markers
    );
    let mut stmt = conn.prepare(&q).unwrap();
    let str_args = [vec![ctx.jurisdiction, ctx.tz].as_slice(), keys.as_slice()].concat();
    let args = str_args
        .iter()
        .map(|v| v as &dyn rusqlite::ToSql)
        .collect::<Vec<_>>();
    let res = stmt.query_map(args.as_slice(), |r| {
        Ok(Ref {
            id: r.get(0)?,
            version: r.get(1)?,
        })
    });
    let refs = res?.collect::<Result<Vec<_>, _>>()?;
    Ok(refs)
}

fn filter(refs: Vec<Ref>) -> Vec<Ref> {
    let max_vers: HashMap<_, _> = refs
        .iter()
        .map(|r| (&r.id, &r.version))
        .fold(HashMap::new(), |mut acc, (id, ver)| {
            acc.entry(id)
                .and_modify(|v: &mut &String| {
                    if v.parse::<u32>().unwrap() < ver.parse::<u32>().unwrap() {
                        *v = ver;
                    }
                })
                .or_insert(ver);
            acc
        });
    refs.iter()
        .filter(|r| max_vers.get(&r.id).copied() == Some(&r.version))
        .cloned()
        .collect()
}

fn rule_path(id: &String, version: &String) -> String {
    let path: PathBuf = [".", "data", "rules", id, &format!("{}.rule", version)]
        .iter()
        .collect();
    path.display().to_string()
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(long)]
    rule_id: String,
    #[arg(long)]
    document: String,
}

fn main() {
    let args = Args::parse();
    let f = std::fs::File::open(&args.document).expect("error opening document");
    let doc: serde_json::Value = serde_json::from_reader(f).expect("parse failure");
    let conn = open_db();
    let ctx = load_context().unwrap();

    let refs = match find(ctx, conn, doc_keys(&doc)) {
        Ok(refs) => filter(refs),
        Err(e) => {
            println!("DB error: {:?}", e);
            return;
        }
    };

    if refs.is_empty() {
        println!("No applicable rules found for this document.");
        return;
    }

    for r in &refs {
        println!("\nEvaluating rule: {} (version {})", r.id, r.version);
        let rp = rule_path(&r.id, &r.version);
        if let Some(rule) = parser::Parse::parse(&rp) {
            let idxs = eval_conds(&rule.conditions, &doc);
            print_results(&rule, &idxs);
        } else {
            println!("  Could not load rule file at {}", rp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keys() {
        let f = std::fs::File::open("test/fixtures/deep_keys.json").expect("error");
        let doc: serde_json::Value = serde_json::from_reader(f).expect("parse failure");
        let ex = vec![
            "a", "a.a0", "a.a0.a00", "a.a0.a01",
            "a.a1", "a.a1.a10", "a.a1.a11", "a.a2",
            "b", "b[0]", "b[0].b00", "b[0].b01",
            "b[1]", "b[1].b10", "b[2]", "b[2].b20",
            "b[2].b20[0]", "b[2].b20[1]",
        ];
        assert_eq!(ex, doc_keys(&doc));
    }
}
