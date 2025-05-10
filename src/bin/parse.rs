use chrono::{ DateTime, TimeZone, NaiveDateTime, Utc };
use chrono_tz::Tz;
use once_cell::sync::Lazy;
use regex::Regex;
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

#[derive(Debug, Clone)]
enum Case {
    False,
    True,
    Maybe,
    Both,
    Invalid,
}

#[derive(Debug)]
enum Op {
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    Unk,
}

#[derive(Debug)]
struct Condition {
    key: String,
    val: String,
    op: Op,
    cases: Vec<Case>,
}

impl Condition {
    fn new(k: &str, v: &str, op: Op, cases: &Vec<Case>) -> Self {
        Condition {
            key: k.to_string(),
            val: v.to_string(),
            op: op,
            cases: cases.to_vec(),
        }
    }
}

#[derive(Debug)]
struct AssertedValue(String, String);

#[derive(Debug)]
struct Assertion {
    vals: Vec<AssertedValue>,
    cases: Vec<Case>,
}

impl Assertion {
    fn new(k: &str, v: &str, cases: &Vec<Case>) -> Self {
        Assertion {
            // only one value is supported right now
            vals: vec![AssertedValue(k.to_string(), v.to_string())],
            cases: cases.to_vec(),
        }
    }
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

static MATCH_ARR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r",\s+").unwrap()
});
static MATCH_COND: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(\w+)(=|!=|<=|>=|<|>)('[\w]+')\s*:\s*\[([\d\s,]+)\]").unwrap()
});
static MATCH_ASSERT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(\w+):=('[\w_]+'):\s*\[([\d\s,]+)\]").unwrap()
});

impl Parse {
    fn new() -> Self {
        Parse { state: ActiveSection::None, rule: Rule::new() }
    }

    fn parse_file(&mut self, fln: &String)  -> io::Result<()> {
        let rdr = BufReader::new(File::open(fln)?);
        for mln in rdr.lines() {
            self.parse_line(&mln?);
        }

        Ok(())
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
        let mut ie = InEffect::new();
        let parts: Vec<_> = MATCH_ARR.split(ln)
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
        if let Some(caps) = MATCH_COND.captures(ln) {
            let k = caps.get(1).unwrap().as_str();
            let op = match caps.get(2).unwrap().as_str() {
                "=" => Op::Eq,
                "!=" => Op::Neq,
                "<" => Op::Lt,
                "<=" => Op::Lte,
                ">" => Op::Gt,
                ">=" => Op::Gte,
                _ => Op::Unk,
            };
            let v = caps.get(3).unwrap().as_str();
            let cases: Vec<_> = MATCH_ARR.split(caps.get(4).unwrap().as_str()).map(|s| {
                match s {
                    "00" => Case::False,
                    "01" => Case::True,
                    "10" => Case::Maybe,
                    "11" => Case::Both,
                    _ => Case::Invalid,
                }
            }).collect();

            self.rule.conditions.push(Condition::new(&k, &v, op, &cases));
        }
    }

    fn parse_line_assert(&mut self, ln: &String) {
        if let Some(caps) = MATCH_ASSERT.captures(ln) {
            let k = caps.get(1).unwrap().as_str();
            let v = caps.get(2).unwrap().as_str();
            let cases: Vec<_> = MATCH_ARR.split(caps.get(3).unwrap().as_str()).map(|s| {
                match s {
                    "00" => Case::False,
                    "01" => Case::True,
                    "10" => Case::Maybe,
                    "11" => Case::Both,
                    _ => Case::Invalid,
                }
            }).collect();

            self.rule.assertions.push(Assertion::new(&k, &v, &cases));
        }
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
    prsr.parse_file(&fln)?;
    println!("{:?}", prsr.rule);

    Ok(())
}
