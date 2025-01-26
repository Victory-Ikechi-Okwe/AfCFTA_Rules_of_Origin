use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() == 2 {
        let f = std::fs::File::open(&args[1]).expect("error");
        let doc: serde_json::Value = serde_json::from_reader(f).expect("parse failure");

	// In etc/contents/default.json we'll find the default values for the jurisdiction. The current time is always calculated
        // "on the machine" by getting the current UTC time and converting it to the timezones in the rules selected from the DB.

	// 1. select everything in the in-effect table and filter according to ^^
        // 2. select everything in the applicable table joined to (1)
        // 3. explode the key-paths in the doc.json
        // 4. intersect (2) and (3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_effect() {
    }
}
