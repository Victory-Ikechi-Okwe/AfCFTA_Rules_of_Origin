use std::env;

// given: doc.json rule in processable form
// output: oughts from rule
//
// processable form:
// - all JSON leaves exploded into a k,v table
fn watch() {
    println!("watching");
}

fn single_run(path: &String, id: &String, rev: u64) {
    println!("single run: path={:?}; id={:?}; rev={:?}", path, id, rev);
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
