use once_cell::sync::Lazy;
use regex::Regex;
use std::env;
use std::fs::File;
use std::io::{ self, BufRead, BufReader };

use rookie::rules::{ Assertion, Case, Condition, InEffect, Op, Rule };

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
            self.rule.add_prop(parts[0], parts[1]);
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
                "IN" => ie.set_jurisdiction(p.1),
                "FROM" => ie.set_from(p.1),
                "TO" => ie.set_to(p.1),
                "TZ" => ie.set_tz(p.1),
                _ => {},
            }
        }

        self.rule.add_in_effect(&ie);
    }

    fn parse_line_cond(&mut self, ln: &String) {
        if let Some(caps) = MATCH_COND.captures(ln) {
            let k = caps.get(1).unwrap().as_str();
            let op = self.parse_op(caps.get(2).unwrap().as_str());
            let v = caps.get(3).unwrap().as_str();
            let cases = self.parse_cases(caps.get(4).unwrap().as_str());

            self.rule.add_cond(Condition::new(&k, &v, op, &cases));
        }
    }

    fn parse_line_assert(&mut self, ln: &String) {
        if let Some(caps) = MATCH_ASSERT.captures(ln) {
            let k = caps.get(1).unwrap().as_str();
            let v = caps.get(2).unwrap().as_str();
            let cases = self.parse_cases(caps.get(3).unwrap().as_str());

            self.rule.add_assert(Assertion::new(&k, &v, &cases));
        }
    }

    fn parse_cases(&self, cs: &str) -> Vec<Case> {
        MATCH_ARR.split(cs).map(|s| {
            match s {
                "00" => Case::False,
                "01" => Case::True,
                "10" => Case::Maybe,
                "11" => Case::Both,
                _ => Case::Invalid,
            }
        }).collect()
    }

    fn parse_op(&self, ops: &str) -> Op {
        match ops {
            "=" => Op::Eq,
            "!=" => Op::Neq,
            "<" => Op::Lt,
            "<=" => Op::Lte,
            ">" => Op::Gt,
            ">=" => Op::Gte,
            _ => Op::Unk,
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
