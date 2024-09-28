use std::{
    env,
    path::PathBuf,
};

// given: doc.json rule in processable form
// output: oughts from rule
//
// processable form:
// - all JSON leaves exploded into a k,v table
fn watch() {
    println!("watching");
}

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

fn single_run(path: &String, id: &String, rev: u64) {
    println!("single run: path={:?}; id={:?}; rev={:?}", path, id, rev);

    let doc_path = PathBuf::from(path);
    let rule_path = rule_dir(&id).join(format!("{:?}.json", rev));

    match [parse_json_file(&doc_path), parse_json_file(&rule_path)] {
        [Some(doc), Some(rule)] => {
            println!("have both: {:?}, {:?}", doc, rule);
        },
        [Some(_), None] => {
            println!("no rule");
        },
        [None, Some(_)] => {
            println!("no doc");
        },
        [None, None] => {
            println!("neither");
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.as_slice() {
        [_] => watch(),
        [_, path, id, rev_s] => {
            let rev = rev_s.parse::<u64>().expect("failed to parse rev");
            single_run(&path, &id, rev);
        }
        _ => {
            println!("invalid args");
        }
    }
}
