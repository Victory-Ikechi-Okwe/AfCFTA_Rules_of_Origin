use std::{
    io::{ self },
    path::PathBuf,
};

use rookie::rules::{
    parser,
    Case,
    Condition,
    Value,
};

fn rule_dir(id: &String) -> PathBuf {
    [".", "data", "rules", id].iter().collect()
}

fn parse_json_file(path: &PathBuf) -> Option<serde_json::Value> {
    let f = std::fs::File::open(path).expect("could not open file");
    match serde_json::from_reader(f) {
        Result::Ok(o) => Some(o),
        Err(_) => None,
    }
}

fn fetch(doc: &serde_json::Value, k: &String) -> String {
    // This needs to handle more types or handle the types natively
    match &doc[k] {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => String::from(if *b { "true" } else { "false" }),
        serde_json::Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn eval_conds(conds: &Vec<Condition>, doc: &serde_json::Value) -> Vec<bool> {
    // TODO: this is limited to 64 scenarios, use BigUint
    let result = conds.iter().fold(u64::MAX, |res_bits, cond| {
        let ac_val = fetch(doc, &cond.key);
        // TODO: the only opt is "eq" - add more???
        let matches = Value::matches(&cond.val, &ac_val);

        println!("ac_val={:?}; val={:?}; matches={:?}", ac_val, cond.val, matches);
        let case_bits = cond.cases.iter().enumerate().fold(0u64, |acc, (i, case)| {
            let b = match case {
                Case::False => !matches,
                Case::True => matches,
                // TODO: figure out maybe
                Case::Maybe => true,
                // TODO: sort of sure this is always true
                Case::Both => true,
                Case::Invalid => false,
            };

            if b { acc | 1 << i } else { acc }
        });

        res_bits & case_bits
    });

    let len = conds.first().unwrap().cases.len();
    println!("result={:#b}; sc_count={:?}", result, len);

    (0..len).map(|i| (result & (1 << i)) > 0).collect()
}

fn single_run(path: &String, id: &String, rev: u64) {
    let doc_path = PathBuf::from(path);
    let rule_path = rule_dir(&id).join(format!("{:?}.rule", rev)).display().to_string();

    println!("single run: path={}; id={}; rev={}; rule_path={:?}", path, id, rev, rule_path);
    if let Some(rule) = parser::Parse::parse(&rule_path) {
        if let Some(doc) = parse_json_file(&doc_path) {
            let e = eval_conds(&rule.conditions, &doc);
            println!("eval={:?}", e);
            // TODO: output assertions
        }
    }
}

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    doc_path: String,
    rule_id: String,
    rule_rev: u64,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    println!("args=={:?}", args);
    single_run(&args.doc_path, &args.rule_id, args.rule_rev);

    Ok(())
}
