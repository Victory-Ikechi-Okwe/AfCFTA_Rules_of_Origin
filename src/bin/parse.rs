use chrono::{ DateTime, TimeZone, NaiveDateTime, Utc };
use chrono_tz::Tz;
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{ self, BufRead, BufReader };
use std::str::FromStr;

#[derive(Debug, Clone)]
struct InEffect {
    jurisdiction: Option<String>,
    from: Option<chrono::DateTime<Utc>>,
    to: Option<chrono::DateTime<Utc>>,
    tz: Option<Tz>,
}

impl InEffect {
    fn new() -> Self {
        InEffect { jurisdiction: None, from: None, to: None, tz: None }
    }
}

#[derive(Debug)]
enum Case {
    True,
    False,
    Maybe,
    Both,
}

#[derive(Debug)]
struct Condition {
    key: String,
    val: String,
    cases: Vec<Case>,
}

#[derive(Debug)]
struct AssertedValue(String, String);

#[derive(Debug)]
struct Assertion {
    asserted_vals: Vec<AssertedValue>,
    cases: Vec<Case>,
}

#[derive(Debug)]
struct Rule {
    props: HashMap<String, String>,
    in_effect: Vec<InEffect>,
    conditions: Vec<Condition>,
    assertions: Vec<Assertion>,
}

impl Rule {
    fn new() -> Self {
        Rule { props: HashMap::new(), in_effect: vec![], conditions: vec![], assertions: vec![] }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ActiveSection {
    None,
    Props,
    Effects,
    Conds,
    Asserts,
}

struct Parse {
    state: ActiveSection,
    rule: Rule,
}

impl Parse {
    fn new() -> Self {
        Parse { state: ActiveSection::None, rule: Rule::new() }
    }

    fn parse_line(&mut self, ln: &String) {
        let ps = ln.split('#').next().unwrap().trim();

        if ps.len() == 0 {
            return;
        }

        let st = match ps {
            "PROPERTIES" => ActiveSection::Props,
            "IN EFFECT" => ActiveSection::Effects,
            "CONDITIONS" => ActiveSection::Conds,
            "ASSERTIONS" => ActiveSection::Asserts,
            _ => self.state.clone(),
        };

        if st == self.state {
            self.parse_line_content(ln);
        } else {
            self.state = st;
        }

        println!("state: {:?}", self.state);
    }

    fn parse_line_content(&mut self, ln: &String) {
        match &self.state {
            ActiveSection::Props => self.parse_line_prop(ln),
            ActiveSection::Effects => self.parse_line_in_effect(ln),
            ActiveSection::Conds => self.parse_line_cond(ln),
            ActiveSection::Asserts => self.parse_line_assert(ln),
            ActiveSection::None => {},
        }
    }

    fn parse_line_prop(&mut self, ln: &String) {
        let parts: Vec<_> = ln.split_whitespace().collect();
        if parts.len() >= 2 {
            self.rule.props.insert(parts[0].to_string(), parts[1].to_string());
        }
    }

    fn parse_line_in_effect(&mut self, ln: &String) {
        let re = regex::Regex::new(r",\s+").unwrap();
        let mut ie = InEffect::new();
        let parts: Vec<_> = re.split(ln)
            .map(|s| {
                let v: Vec<_> = s.split_whitespace().collect();

                match v.len() {
                    0 => ("", ""),
                    1 => (v[0], ""),
                    _ => (v[0], v[1]),
                }
            }).collect();

        for p in parts {
            match p.0 {
                "IN" => ie.jurisdiction = Some(p.1.to_string()),
                "FROM" => {
                    let ndt = NaiveDateTime::parse_from_str(p.1, "%Y-%m-%dT%H:%M").expect("failed parse");
                    ie.from = Some(DateTime::from_utc(ndt, Utc));
                },
                "TO" => {
                    let ndt = NaiveDateTime::parse_from_str(p.1, "%Y-%m-%dT%H:%M").expect("failed parse");
                    ie.to = Some(DateTime::from_utc(ndt, Utc));
                },
                "TZ" => {
                    ie.tz = Some(Tz::from_str(p.1).expect("unknown tz"));
                },
                _ => {},
            }
        }

        self.rule.in_effect.push(ie.clone());
    }

    fn parse_line_cond(&mut self, ln: &String) {
    }

    fn parse_line_assert(&mut self, ln: &String) {
    }
}

fn main() -> io::Result<()> {
    let fln = match env::args().nth(1) {
        Some(a) => a,
        None => {
            eprintln!("give me a file name");
            std::process::exit(1);
        }
    };

    let mut prsr = Parse::new();

    let rdr = BufReader::new(File::open(&fln)?);
    for mln in rdr.lines() {
        prsr.parse_line(&mln?);
    }

    println!("{:?}", prsr.rule);

    Ok(())
}
