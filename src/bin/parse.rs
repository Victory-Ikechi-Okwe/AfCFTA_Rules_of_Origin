use rookie::rules::{parser, parser::RulesetParser};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    rules_fn: String,
}

fn main() {
    let args = Args::parse();

    if let Some(rule) = parser::Parse::parse(&args.rules_fn) {
        println!("{:?}", rule);
    } else {
        println!("parse failed");
    }
}
