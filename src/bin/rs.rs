use rookie::rules::{eval_conds, print_results, parser, parser::RulesetParser};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
};

#[derive(Deserialize)]
struct RtPackage {
    rule_id: String,
    document: serde_json::Value,
}

#[derive(Serialize)]
struct AssertionResult {
    key: String,
    outcome: String,
}

#[derive(Serialize)]
struct RsResponse {
    rule_id: String,
    version: String,
    scenario: String,
    assertions: Vec<AssertionResult>,
}

fn open_db() -> Connection {
    Connection::open("data/rules.db").expect("failed to open rules.db")
}

fn find_latest_version(conn: &Connection, rule_id: &str) -> Option<String> {
    let mut stmt = conn
        .prepare(
            "SELECT version FROM in_effect WHERE rule_id=? ORDER BY CAST(version AS INTEGER) DESC LIMIT 1",
        )
        .ok()?;
    stmt.query_row([rule_id], |r| r.get(0)).ok()
}

fn rule_path(id: &str, version: &str) -> String {
    let path: PathBuf = [".", "data", "rules", id, &format!("{}.rule", version)]
        .iter()
        .collect();
    path.display().to_string()
}

fn evaluate(pkg: RtPackage) -> Option<RsResponse> {
    let conn = open_db();
    let version = find_latest_version(&conn, &pkg.rule_id)?;
    let rp = rule_path(&pkg.rule_id, &version);
    let rule = parser::Parse::parse(&rp)?;

    let idxs = eval_conds(&rule.conditions, &pkg.document);
    let case_labels = ["A", "B", "C", "D", "E", "F", "G", "H"];

    let scenario = if idxs.is_empty() {
        "No matching scenario".to_string()
    } else {
        idxs.iter()
            .map(|i| format!("Case {}", case_labels.get(*i).unwrap_or(&"?")))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let assertions = rule
        .assertions
        .iter()
        .flat_map(|asrt| {
            let reduced = asrt.reduce(&idxs);
            asrt.vals.iter().zip(reduced.cases.iter()).map(|(val, case)| {
                let outcome = match case {
                    rookie::rules::Case::True => "TRUE",
                    rookie::rules::Case::False => "FALSE",
                    rookie::rules::Case::Maybe => "MAYBE",
                    rookie::rules::Case::Both => "TRUE (and possibly false)",
                    rookie::rules::Case::Invalid => "INVALID",
                };
                AssertionResult {
                    key: val.0.clone(),
                    outcome: outcome.to_string(),
                }
            })
            .collect::<Vec<_>>()
        })
        .collect();

    Some(RsResponse {
        rule_id: pkg.rule_id,
        version,
        scenario,
        assertions,
    })
}

#[tokio::main]
async fn main() {
    let socket_path = "/tmp/rs.sock";

    if std::path::Path::new(socket_path).exists() {
        std::fs::remove_file(socket_path).unwrap();
    }

    let listener = UnixListener::bind(socket_path).expect("failed to bind socket");
    println!("Rule Reserve listening on {}", socket_path);

    loop {
        let (mut stream, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }

            let response = match serde_json::from_slice::<RtPackage>(&buf[..n]) {
                Ok(pkg) => match evaluate(pkg) {
                    Some(result) => serde_json::to_vec(&result).unwrap(),
                    None => b"{\"error\": \"rule not found or evaluation failed\"}".to_vec(),
                },
                Err(e) => format!("{{\"error\": \"bad request: {}\"}}", e).into_bytes(),
            };

            stream.write_all(&response).await.ok();
            stream.flush().await.ok();
        });
    }
}
