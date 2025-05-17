use std::env;
use std::io::{ self };

use rookie::rules::parser;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    rules_fn: String,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let mut prsr = parser::Parse::new();
    prsr.parse_file(&args.rules_fn)?;
    println!("{:?}", prsr.rule);

    Ok(())
}
