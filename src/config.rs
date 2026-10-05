use crate::clones::CloneConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize, Clone)]
#[serde(default, rename_all = "kebab-case")]
pub struct Limits {
    pub cyclomatic: u32,
    pub cognitive: u32,
    pub function_sloc: u32,
    pub nesting: u32,
    pub params: u32,
    pub if_ladder: u32,
    pub condition_ops: u32,
    pub file_sloc: u32,
    pub call_depth: u32,
    pub magic_numbers: u32,
}

impl Default for Limits {
    // sloplint: ignore[magic-numbers] this table is where the limits get their names
    fn default() -> Self {
        Limits {
            cyclomatic: 15,
            cognitive: 20,
            function_sloc: 80,
            nesting: 4,
            params: 6,
            if_ladder: 4,
            condition_ops: 3,
            file_sloc: 1000,
            call_depth: 10,
            magic_numbers: 8,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Default)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProjectLimits {
    pub max_slop_score: Option<f64>,
}

#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Error,
    Warn,
    Off,
}

#[derive(Deserialize, Serialize, Clone, Default)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
    pub exclude: Vec<String>,
    pub include_tests: bool,
    pub limits: Limits,
    pub clones: CloneConfig,
    pub project: ProjectLimits,
    pub rules: BTreeMap<String, Level>,
}

const ERROR_BY_DEFAULT: &[&str] = &["magic-numbers", "hardcoded-color", "error-only-printed"];
const OFF_BY_DEFAULT: &[&str] = &["return-staircase", "deep-call-chain"];

impl Config {
    pub fn load(explicit: Option<&Path>) -> Result<Config> {
        let path = match explicit {
            Some(p) => Some(p.to_path_buf()),
            None => find_config(),
        };
        let Some(path) = path else { return Ok(Config::default()) };
        let raw = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn level(&self, rule: &str) -> Level {
        let default = if ERROR_BY_DEFAULT.contains(&rule) {
            Level::Error
        } else if OFF_BY_DEFAULT.contains(&rule) {
            Level::Off
        } else {
            Level::Warn
        };
        self.rules.get(rule).copied().unwrap_or(default)
    }
}

fn find_config() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        for name in ["sloplint.toml", ".sloplint.toml"] {
            if dir.join(name).is_file() {
                return Some(dir.join(name));
            }
        }
        if dir.join(".git").exists() || !dir.pop() {
            return None;
        }
    }
}
