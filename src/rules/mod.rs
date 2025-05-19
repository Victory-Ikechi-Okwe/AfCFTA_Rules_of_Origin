use chrono::{ DateTime, NaiveDateTime, Utc };
use chrono_tz::Tz;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::str::FromStr;

pub mod parser;

#[derive(Debug, Clone)]
pub struct InEffect {
    pub jurisdiction: Option<String>,
    pub from: Option<chrono::DateTime<Utc>>,
    pub to: Option<chrono::DateTime<Utc>>,
    pub tz: Option<Tz>,
}

fn dt_from_str(s: &str) -> Option<chrono::DateTime<Utc>> {
    match NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M") {
        Ok(dt) => Some(DateTime::from_utc(dt, Utc)),
        _ => None,
    }
}

impl InEffect {
    pub fn new() -> Self {
        InEffect { jurisdiction: None, from: None, to: None, tz: None }
    }

    pub fn set_jurisdiction(&mut self, v: &str) {
        self.jurisdiction = Some(v.to_string());
    }

    pub fn set_from(&mut self, v: &str) {
        self.from = dt_from_str(v);
    }

    pub fn set_to(&mut self, v: &str) {
        self.to = dt_from_str(v);
    }

    pub fn set_tz(&mut self, v: &str) {
        self.tz = match Tz::from_str(v) {
            Ok(tz) => Some(tz),
            _ => None,
        };
    }
}

#[derive(Debug, Clone)]
pub enum Case {
    False,
    True,
    Maybe,
    Both,
    Invalid,
}

#[derive(Clone, Debug)]
pub enum Op {
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    Unk,
}

#[derive(Clone, Debug)]
pub enum Value {
    Ord(u64),
    Str(String),
    Invalid,
}

static MATCH_STR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^'(.+)'$").unwrap()
});
static MATCH_ORD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(\d+)$").unwrap()
});

impl Value {
    fn parse(v: &str) -> Value {
        if let Some(caps) = MATCH_STR.captures(v) {
            Value::Str(caps.get(1).unwrap().as_str().to_string())
        } else if let Some(caps) = MATCH_ORD.captures(v) {
            Value::Ord(caps.get(1).unwrap().as_str().parse::<u64>().unwrap())
        } else {
            Value::Invalid
        }
    }

    pub fn matches(v: &Value, to_match: &String) -> bool {
        match v {
            Value::Str(s) => s == to_match,
            Value::Ord(o) => *o == to_match.parse::<u64>().unwrap(),
            Value::Invalid => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Condition {
    pub key: String,
    pub val: Value,
    pub op: Op,
    pub cases: Vec<Case>,
}

impl Condition {
    pub fn new(k: &str, v: &str, op: Op, cases: &Vec<Case>) -> Self {
        Condition {
            key: k.to_string(),
            val: Value::parse(v),
            op: op,
            cases: cases.to_vec(),
        }
    }
}

#[derive(Clone, Debug)]
struct AssertedValue(String, String);

#[derive(Clone, Debug)]
pub struct Assertion {
    vals: Vec<AssertedValue>,
    cases: Vec<Case>,
}

impl Assertion {
    pub fn new(k: &str, v: &str, cases: &Vec<Case>) -> Self {
        Assertion {
            // only one value is supported right now
            vals: vec![AssertedValue(k.to_string(), v.to_string())],
            cases: cases.to_vec(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Rule {
    props: HashMap<String, String>,
    pub in_effect: Vec<InEffect>,
    pub conditions: Vec<Condition>,
    assertions: Vec<Assertion>,
}

impl Rule {
    pub fn new() -> Self {
        Rule { props: HashMap::new(), in_effect: vec![], conditions: vec![], assertions: vec![] }
    }

    pub fn add_in_effect(&mut self, ie: &InEffect) {
        self.in_effect.push(ie.clone());
    }

    pub fn add_prop(&mut self, k: &str, v: &str) {
        self.props.insert(k.to_string(), v.to_string());
    }

    pub fn add_cond(&mut self, c: Condition) {
        self.conditions.push(c.clone());
    }

    pub fn add_assert(&mut self, a: Assertion) {
        self.assertions.push(a.clone());
    }

    pub fn refine(&mut self) {
        self.props.entry(String::from("ID")).or_insert(uuid::Uuid::new_v4().hyphenated().to_string());
    }

    pub fn prop(&self, k: &str) -> Option<String> {
        self.props.get(&String::from(k)).map(|v| v.clone())
    }

    pub fn id(&self) -> String {
        self.prop("ID").expect("ID should have been generated")
    }
}
