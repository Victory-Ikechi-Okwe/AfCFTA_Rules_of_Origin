use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() == 1 {
        dbg!("watch");
    } else {
        dbg!(&args[1]);
    }
}
