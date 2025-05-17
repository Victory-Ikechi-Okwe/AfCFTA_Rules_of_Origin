use std::env;
use std::io::{ self };

use rookie::rules::parser;

fn main() -> io::Result<()> {
    let fln = match env::args().nth(1) {
        Some(a) => a,
        None => {
            eprintln!("give me a file name");
            std::process::exit(1);
        }
    };

    let mut prsr = parser::Parse::new();
    prsr.parse_file(&fln)?;
    println!("{:?}", prsr.rule);

    Ok(())
}
