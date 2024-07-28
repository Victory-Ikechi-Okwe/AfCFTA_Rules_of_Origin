use log::*;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.as_slice() {
        [_, path, id] => {
            println!("path={:?}; id={:?}", path, id);
        },
        [_, path] => {
            println!("path={:?}", path);
        },
        _ => {
            println!("invalid args");
        }
    }
}
