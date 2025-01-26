use std::env;

fn doc_keys(doc: &serde_json::Value) -> Vec<String> {
    keys(doc, true)
}

fn keys(doc: &serde_json::Value, at_root: bool) -> Vec<String> {
    match doc {
        serde_json::Value::Object(o) => {
            let mut v = vec![];
            for (k, val) in o {
                v.push(k.clone());
                v.extend(keys(val, false).iter().map(|s| format!("{}{}", k, s)));
            }
            if at_root {
                v
            } else {
                v.iter().map(|s| format!(".{}", s)).collect()
            }
       },
       serde_json::Value::Array(a) => {
           let mut v = vec![];
           for (i, it) in a.iter().enumerate() {
               v.push(format!("[{}]", i));
               v.extend(keys(it, false).iter().map(|s| format!("[{}]{}", i, s)));
           }
           v
       },
       _ => {
           vec![]
       }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() == 2 {
        let f = std::fs::File::open(&args[1]).expect("error");
        let doc: serde_json::Value = serde_json::from_reader(f).expect("parse failure");

//        keys(&doc, None);
	// In etc/contents/default.json we'll find the default values for the jurisdiction. The current time is always calculated
        // "on the machine" by getting the current UTC time and converting it to the timezones in the rules selected from the DB.

	// 1. select everything in the in-effect table and filter according to ^^
        // 2. select everything in the applicable table joined to (1)
        // 3. explode the key-paths in the doc.json
        // 4. intersect (2) and (3)
        //
        // - we could consider performing (3) when the document is retained - using a different bin invoked from the api. likely
        //   this would form part of a yet-unspecified queue tool (bin/q, bin/dq -> bin/q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keys() {
        let f = std::fs::File::open("test/fixtures/deep_keys.json").expect("error");
        let doc: serde_json::Value = serde_json::from_reader(f).expect("parse failure");

        let ex = vec![
            "a",
            "a.a0",
            "a.a0.a00",
            "a.a0.a01",
            "a.a1",
            "a.a1.a10",
            "a.a1.a11",
            "a.a2",
            "b",
            "b[0]",
            "b[0].b00",
            "b[0].b01",
            "b[1]",
            "b[1].b10",
            "b[2]",
            "b[2].b20",
            "b[2].b20[0]",
            "b[2].b20[1]"
        ];

        assert_eq!(ex, doc_keys(&doc));
    }
}
