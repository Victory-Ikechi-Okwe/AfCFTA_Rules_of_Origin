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

#[derive(Debug, Clone)]
enum Scenario {
    No,
    Yes,
    Maybe,
    Both,
    Invalid,
}


#[derive(Debug, Clone)]
struct InputCond {
    pub key: String,
    pub val: String,
    pub op: String,
    pub sc: Vec<Scenario>,
}

fn parse_scenarios(vals: &serde_json::Value) -> Option<Vec<Scenario>> {
    match &vals {
        serde_json::Value::Array(scenarios) => {
            Some(scenarios.iter().map(|v| match v {
                serde_json::Value::String(s) => {
                    match s.as_str() {
                        "00" => Scenario::No,
                        "01" => Scenario::Yes,
                        "10" => Scenario::Maybe,
                        "11" => Scenario::Both,
                        _ => Scenario::Invalid,
                    }
                },
                _ => Scenario::Invalid,
            }).collect())
        },
        _ => None
    }
}

fn parse_input_conditions(vals: &serde_json::Value) -> Option<Vec<InputCond>> {
    match &vals["input_conditions"] {
        serde_json::Value::Array(cond_vals) => {
            let conds: Vec<InputCond> = cond_vals.iter().map(|v| match v {
                serde_json::Value::Object(cond_o) => {
                    let scenarios = parse_scenarios(&cond_o["scenarios"]);

                    Some(InputCond {
                        key: cond_o["expression"]["key"].as_str().unwrap().to_string(),
                        val: cond_o["expression"]["value"].as_str().unwrap().to_string(),
                        op: cond_o["expression"]["op"].as_str().unwrap().to_string(),
                        sc: match &scenarios {
                            Some(v) => v.clone(),
                            None => Vec::new(),
                        },
                    })
                },
                _ => None
            }).flatten().collect();

            Some(conds)
        },
        _ => {
            println!("no input conditions");
            None
        }
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

fn eval_conds(conds: Vec<InputCond>, doc: &serde_json::Value) -> Vec<bool> {
    // TODO: this is limited to 64 scenarios, use BigUint
    let result = conds.iter().fold(u64::MAX, |res_bits, cond| {
        let ac_val = fetch(doc, &cond.key);
        // TODO: the only opt is "eq" - add more???
        let matches = ac_val == cond.val;

        println!("ac_val={:?}; val={:?}; matches={:?}", ac_val, cond.val, matches);
        let sc_bits = cond.sc.iter().enumerate().fold(0u64, |acc, (i, sc)| {
            let b = match sc {
                Scenario::No => !matches,
                Scenario::Yes => matches,
                // TODO: figure out maybe
                Scenario::Maybe => true,
                // TODO: sort of sure this is always true
                Scenario::Both => true,
                Scenario::Invalid => false,
            };

            if b { acc | 1 << i } else { acc }
        });

        res_bits & sc_bits
    });

    // TODO: build the vector
    let len = conds.first().unwrap().sc.len();
    println!("result={:#b}; sc_count={:?}", result, len);

    (0..len).map(|i| (result & (1 << i)) > 0).collect()
}

fn single_run(path: &String, id: &String, rev: u64) {
    println!("single run: path={:?}; id={:?}; rev={:?}", path, id, rev);

    let doc_path = PathBuf::from(path);
    let rule_path = rule_dir(&id).join(format!("{:?}.json", rev));

    match [parse_json_file(&doc_path), parse_json_file(&rule_path)] {
        [Some(doc), Some(rule)] => {
            match parse_input_conditions(&rule) {
                Some(conds) => {
                    let e = eval_conds(conds, &doc);
                    println!("eval={:?}", e);
                    // TODO: match to output assertions
                },
                None => {
                    println!("empty conds");
                }
            }

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
