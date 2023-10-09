use glob::glob;
use log::*;
use std::{
    path::PathBuf,
};

// REFACTOR: we have the id, which should be the basis for a Rule instance, but history
// led us to this point. We should remove the PathBuf Rule construction for something
// based on Rule { id: } e.g. make_rule(id) which would be populated using something
// like the code below
// generally: all the functions that take PathBuf and look around in `data/rules` should
// be rewritten in terms of Rule { id: }
// private
fn extract_rev(p: &PathBuf) -> u64 {
    match p.as_path().file_stem() {
        None => 0,
        Some(st) => { st.to_str().unwrap().parse().unwrap() }
    }
}

#[derive(Debug)]
pub struct Rule {
    path: PathBuf,
    dir: PathBuf,
    pub id: String,
    pub rev: u64,
}

impl Rule {
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn publish(&self) -> bool {
        let target = self.path.parent().unwrap().join("published");

        info!("storing (rule={:?})", self);
        std::fs::remove_file(&target).expect("failed to remove target");

        match std::os::unix::fs::symlink(&self.path, &target) {
            Ok(_) => {
                debug!("linked (path={:?}, target={:?}", self.path, target);
                true
            },
            _ => {
                debug!("failed link (path={:?}, target={:?}", self.path, target);
                false
            }
        }
    }

    pub fn store(&self, d: &serde_json::Map<String, serde_json::Value>) -> bool {
        debug!("writing rule (rule={:?})", self);
        match std::fs::File::create(&self.path) {
            Ok(f) => {
                match serde_json::to_writer(f, d) {
                    Ok(_) => {
                        debug!("wrote rule (rule={:?}", self);
                        true
                    },
                    Err(e) => {
                        debug!("failed to write rule (rule={:?}; e={:?})", self, e);
                        false
                    }
                }
            },
            Err(e) => {
                debug!("failed to create file (rule={:?}; e={:?}", self, e);
                false
            }
        }
    }
}

pub fn rule_dir(id: &String) -> PathBuf {
    [".", "data", "rules", id].iter().collect()
}

pub fn rule_path(id: &String, rev: u64) -> PathBuf {
    rule_dir(id).join(format!("{:?}.json", rev))
}

pub fn find_rule_by_rev(id: &String, rev: u64) -> Option<Rule> {
    let path = rule_path(id, rev);
    if path.exists() {
        Some(Rule { path: path.clone(), dir: rule_dir(id).clone(), id: id.to_string(), rev: rev })
    } else {
        None
    }
}

pub fn find_latest_rule(id: &String) -> Option<Rule> {
    let dir = rule_dir(id);
    let vers = dir.join("*.json");

    debug!("searching for rules: vers={:?}", vers);
    let po = match glob(vers.to_str().unwrap()) {
        Ok(it) => it.filter_map(|p| p.ok()).max_by_key(extract_rev),
        _ => None
    };

    match po {
        Some(ref p) => Some(Rule { path: p.clone(), dir: dir.clone(), id: id.clone(), rev: extract_rev(&p) }),
        None => None,
    }
}

pub fn next_revision(id: &String) -> Rule {
    match find_latest_rule(id) {
        Some(rule) => {
            debug!("found latest rule (rule={:?})", rule);
            let rev = rule.rev + 1;
            let path = rule.dir.join(format!("{:?}.json", rev));

            Rule { path: path.clone(), dir: rule.dir.clone(), id: rule.id.clone(), rev: rule.rev + 1 }
        },
        None => {
            let dir = rule_dir(id);

            match std::fs::create_dir_all(&dir) {
                Err(e) => debug!("failed to create store dir (dir={:?}, e={:?}", dir, e),
                _ => { }
            };

            Rule { path: dir.join("1.json"), dir: dir.clone(), id: id.clone(), rev: 1 }
        }
    }
}
