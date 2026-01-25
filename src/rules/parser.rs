use log::error;
use once_cell::sync::Lazy;
use regex::Regex;
use std::fs::File;
use std::io::{self, BufRead, BufReader};

use crate::rules::{Assertion, Case, Condition, InEffect, Op, Rule};

pub use self::IORParser as Parse;

#[derive(Debug)]
pub enum ParseError {
    IoError(std::io::Error),
    InvalidFormat(String),
}

pub trait RulesetParser {
    fn parse(file_path: &String) -> Option<Rule>
    where
        Self: Sized,
    {
        match File::open(file_path) {
            Ok(file) => {
                let mut reader = BufReader::new(file);
                Self::parse_reader(&mut reader)
            }
            Err(e) => {
                error!(
                    "error reading file '{}' using format {}: {:?}",
                    file_path,
                    Self::format_name(),
                    e
                );
                None
            }
        }
    }

    fn format_name() -> &'static str
    where
        Self: Sized,
    {
        ""
    }

    fn supports_file(_file_path: &str) -> bool
    where
        Self: Sized,
    {
        true
    }

    fn parse_str(content: &str) -> Option<Rule>
    where
        Self: Sized,
    {
        let cursor = std::io::Cursor::new(content);
        let mut reader = BufReader::new(cursor);
        Self::parse_reader(&mut reader)
    }

    fn parse_reader<R: BufRead>(reader: &mut R) -> Option<Rule>
    where
        Self: Sized;
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ActiveSection {
    None,
    Props,
    Effects,
    Conds,
    Asserts,
}

pub struct IORParser {
    state: ActiveSection,
    pub rule: Rule,
}

static MATCH_ARR: Lazy<Regex> = Lazy::new(|| Regex::new(r",\s+").unwrap());
static MATCH_COND: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(\w+)(=|!=|<=|>=|<|>)('[\w.]+')\s*:\s*\[([\d\s,]+)\]").unwrap());
static MATCH_ASSERT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(\w+):=('[\w_]+'):\s*\[([\d\s,]+)\]").unwrap());

impl Default for IORParser {
    fn default() -> Self {
        Self::new()
    }
}

impl IORParser {
    pub fn new() -> Self {
        IORParser {
            state: ActiveSection::None,
            rule: Rule::new(),
        }
    }

    pub fn parse_file(&mut self, fln: &String) -> io::Result<()> {
        let rdr = BufReader::new(File::open(fln)?);
        for mln in rdr.lines() {
            self.parse_line(&mln?);
        }

        self.rule.refine();

        Ok(())
    }

    fn parse_line(&mut self, ln: &String) {
        let ps = ln.split('#').next().unwrap().trim();

        if ps.is_empty() {
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
            ActiveSection::None => {}
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
        let parts: Vec<_> = MATCH_ARR
            .split(ln)
            .map(|s| {
                let v: Vec<_> = s.split_whitespace().collect();

                match v.len() {
                    0 => ("", ""),
                    1 => (v[0], ""),
                    _ => (v[0], v[1]),
                }
            })
            .collect();

        for p in parts {
            match p.0 {
                "IN" => ie.set_jurisdiction(p.1),
                "FROM" => ie.set_from(p.1),
                "TO" => ie.set_to(p.1),
                "TZ" => ie.set_tz(p.1),
                _ => {}
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

            self.rule.add_cond(Condition::new(k, v, op, &cases));
        }
    }

    fn parse_line_assert(&mut self, ln: &String) {
        if let Some(caps) = MATCH_ASSERT.captures(ln) {
            let k = caps.get(1).unwrap().as_str();
            let v = caps.get(2).unwrap().as_str();
            let cases = self.parse_cases(caps.get(3).unwrap().as_str());

            self.rule.add_assert(Assertion::new(k, v, &cases));
        }
    }

    fn parse_cases(&self, cs: &str) -> Vec<Case> {
        MATCH_ARR
            .split(cs)
            .map(|s| match s {
                "00" => Case::False,
                "01" => Case::True,
                "10" => Case::Maybe,
                "11" => Case::Both,
                _ => Case::Invalid,
            })
            .collect()
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

// // Compatibility wrapper for existing code
// impl IORParser {
//     pub fn parse(file_path: &String) -> Option<Rule> {
//         IORParser::parse(file_path)
//     }
// }

impl RulesetParser for IORParser {
    fn format_name() -> &'static str {
        "IOR"
    }

    fn supports_file(file_path: &str) -> bool {
        file_path.ends_with(".ior") || file_path.ends_with(".rule")
    }

    fn parse_reader<R: BufRead>(reader: &mut R) -> Option<Rule> {
        let mut prsr = Self::new();
        for line in reader.lines() {
            prsr.parse_line(&line.ok()?);
        }
        prsr.rule.refine();
        Some(prsr.rule.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ior_parser_format_detection() {
        assert!(IORParser::supports_file("test.rule"));
        assert!(IORParser::supports_file("test.ior"));
        assert!(!IORParser::supports_file("test.json"));
        assert_eq!(IORParser::format_name(), "IOR");
    }

    #[test]
    fn test_parse_str_minimal() {
        let content = r#"PROPERTIES
ID test-rule
"#;

        let rule = IORParser::parse_str(content);
        assert!(rule.is_some());
        assert_eq!(rule.unwrap().id(), "test-rule");
    }

    #[test]
    fn test_parse_str_conditions_with_operators() {
        let content = r#"PROPERTIES
ID condition-test

CONDITIONS
status='active': [01, 11]
count!='0': [00, 10]
value<='100': [01, 01, 00]
rate>='0.5': [11, 00]
threshold<'10': [10, 01]
priority>'5': [01, 10]
"#;

        let rule = IORParser::parse_str(content);
        assert!(rule.is_some());
        let parsed_rule = rule.unwrap();
        assert_eq!(parsed_rule.id(), "condition-test");
        assert_eq!(parsed_rule.conditions.len(), 6);
    }

    #[test]
    fn test_parse_str_assertions() {
        let content = r#"PROPERTIES
ID assertion-test

ASSERTIONS
result:='success': [01, 01, 00]
action:='pending': [00, 11, 01]
status:='complete': [11, 00, 10]
"#;

        let rule = IORParser::parse_str(content);
        assert!(rule.is_some());
        let parsed_rule = rule.unwrap();
        assert_eq!(parsed_rule.id(), "assertion-test");
        assert_eq!(parsed_rule.assertions.len(), 3);
    }

    #[test]
    fn test_parse_str_in_effect() {
        let content = r#"PROPERTIES
ID in-effect-test

IN EFFECT
IN CA-QC, FROM 2021-04-01T00:00, TO 2022-03-31T23:59, TZ Canada/Eastern
IN US-CA, FROM 2020-01-01T00:00, TO 2020-12-31T23:59, TZ America/Los_Angeles
"#;

        let rule = IORParser::parse_str(content);
        assert!(rule.is_some());
        let parsed_rule = rule.unwrap();
        assert_eq!(parsed_rule.id(), "in-effect-test");
        assert_eq!(parsed_rule.in_effect.len(), 2);
    }

    #[test]
    fn test_parse_str_all_case_values() {
        let content = r#"PROPERTIES
ID case-test

CONDITIONS
flag='test': [00, 01, 10, 11, 00, 01]
"#;

        let rule = IORParser::parse_str(content);
        assert!(rule.is_some());
        assert_eq!(rule.unwrap().id(), "case-test");
    }

    #[test]
    fn test_parse_str_comments_and_empty_lines() {
        let content = r#"# This is a header comment
PROPERTIES
# ID comment
ID comment-test # inline comment

# Empty line below

CONDITIONS
# Comment before condition
status='active': [01, 11] # inline comment

# Empty line above assertion

ASSERTIONS
result:='ok': [01]
"#;

        let rule = IORParser::parse_str(content);
        assert!(rule.is_some());
        let parsed_rule = rule.unwrap();
        assert_eq!(parsed_rule.id(), "comment-test");
        assert_eq!(parsed_rule.conditions.len(), 1);
        assert_eq!(parsed_rule.assertions.len(), 1);
    }

    #[test]
    fn test_parse_str_docks_ior() {
        let content = r#"PROPERTIES
  ID e059d4b8-a992-4da9-aa2f-8dafcd335cba

IN EFFECT
  IN CA-QC, FROM 2021-04-01T00:00, TO 2022-03-31T23:59, TZ Canada/Eastern
  IN CA-NS, FROM 2021-04-01T00:00, TO 2022-03-31T23:59, TZ Canada/Atlantic

# input conditions
CONDITIONS
  # another comment
  container_status='loaded':         [00, 11, 00, 01, 01, 01, 00, 10]
  validation='inspected':            [00, 11, 11, 00, 01, 00, 01, 01]
  door_status='locked':              [11, 01, 00, 01, 00, 00, 01, 01]
# output assertions
ASSERTIONS
  risk_code:='green':                [01, 01, 00, 00, 00, 00, 01, 01]
  risk_code:='yellow':               [00, 00, 01, 00, 01, 01, 00, 00]
  risk_code:='red':                  [00, 00, 00, 01, 00, 00, 00, 00]
  stevedore:='notified':             [11, 10, 11, 01, 01, 01, 00, 10]
  insurance:='errors_and_omissions': [00, 01, 01, 10, 01, 00, 00, 01]
"#;

        let rule = IORParser::parse_str(content);
        assert!(rule.is_some());
        let parsed_rule = rule.unwrap();

        assert_eq!(parsed_rule.id(), "e059d4b8-a992-4da9-aa2f-8dafcd335cba");
        assert_eq!(parsed_rule.in_effect.len(), 2);
        assert_eq!(parsed_rule.conditions.len(), 3);
        assert_eq!(parsed_rule.assertions.len(), 5);
    }

    #[test]
    fn test_parse_str_multiple_properties() {
        let content = r#"PROPERTIES
ID multi-prop-test
VERSION 1
AUTHOR test-user
DESCRIPTION Test rule with multiple properties
CREATED 2024-01-01
"#;

        let rule = IORParser::parse_str(content);
        assert!(rule.is_some());
        assert_eq!(rule.unwrap().id(), "multi-prop-test");
    }

    #[test]
    fn test_parse_file_nonexistent() {
        let rule = IORParser::parse(&"nonexistent.file".to_string());
        assert!(rule.is_none());
    }

    #[test]
    fn test_parse_file_valid_minimal() {
        let test_content = r#"PROPERTIES
ID minimal-test
"#;

        let file_path = "test_minimal.rule";
        std::fs::write(file_path, test_content).unwrap();

        let rule = IORParser::parse(&file_path.to_string());
        assert!(rule.is_some());
        assert_eq!(rule.unwrap().id(), "minimal-test");

        std::fs::remove_file(file_path).unwrap();
    }

    #[test]
    fn test_parse_file_invalid_content() {
        let test_content = r#"PROPERTIES
ID invalid-test
INVALID_SECTION
not a valid line
"#;

        let file_path = "test_invalid.rule";
        std::fs::write(file_path, test_content).unwrap();

        let rule = IORParser::parse(&file_path.to_string());
        assert!(rule.is_some());
        let parsed_rule = rule.unwrap();

        // Only valid content should be parsed
        assert_eq!(parsed_rule.id(), "invalid-test");
        // "INVALID_SECTION" has only 1 token, not added as property
        // "not a valid line" adds prop "not" -> "a"
        assert_eq!(parsed_rule.prop("not"), Some("a".to_string()));

        std::fs::remove_file(file_path).unwrap();
    }

    #[test]
    fn test_parse_file_examples() {
        let rule = IORParser::parse(&"examples/docks.ior".to_string());
        assert!(rule.is_some());
        let parsed_rule = rule.unwrap();
        assert_eq!(parsed_rule.id(), "e059d4b8-a992-4da9-aa2f-8dafcd335cba");
        assert_eq!(parsed_rule.in_effect.len(), 2);
        assert_eq!(parsed_rule.conditions.len(), 3);
        assert_eq!(parsed_rule.assertions.len(), 5);
    }

    #[test]
    fn test_parse_file_no_id_example() {
        let rule = IORParser::parse(&"examples/docks.no_id.ior".to_string());
        assert!(rule.is_some());
        let parsed_rule = rule.unwrap();
        assert_eq!(parsed_rule.in_effect.len(), 2);
        assert_eq!(parsed_rule.conditions.len(), 3);
        assert_eq!(parsed_rule.assertions.len(), 5);
    }

    #[test]
    fn test_parse_reader_basic() {
        let content = r#"PROPERTIES
ID reader-test

CONDITIONS
status='active': [01, 11]

ASSERTIONS
result:='ok': [01]
"#;

        let cursor = std::io::Cursor::new(content);
        let mut reader = BufReader::new(cursor);
        let rule = IORParser::parse_reader(&mut reader);
        assert!(rule.is_some());
        let parsed_rule = rule.unwrap();
        assert_eq!(parsed_rule.id(), "reader-test");
        assert_eq!(parsed_rule.conditions.len(), 1);
        assert_eq!(parsed_rule.assertions.len(), 1);
    }
}
