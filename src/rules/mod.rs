use chrono::{ DateTime, NaiveDateTime, Utc };
use chrono_tz::Tz;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct InEffect {
    jurisdiction: Option<String>,
    from: Option<chrono::DateTime<Utc>>,
    to: Option<chrono::DateTime<Utc>>,
    tz: Option<Tz>,
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
pub struct Condition {
    key: String,
    val: String,
    op: Op,
    cases: Vec<Case>,
}

impl Condition {
    pub fn new(k: &str, v: &str, op: Op, cases: &Vec<Case>) -> Self {
        Condition {
            key: k.to_string(),
            val: v.to_string(),
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

#[derive(Debug)]
pub struct Rule {
    props: HashMap<String, String>,
    in_effect: Vec<InEffect>,
    conditions: Vec<Condition>,
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
}
