use crate::analyze::{FileFacts, Function};
use crate::callgraph::Chain;
use crate::clones::Duplicates;
use crate::config::{Config, Level};
use crate::rules::VERBOSITY_RULES;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Clone)]
pub struct Finding {
    pub rule: String,
    pub level: Level,
    pub path: String,
    pub start: u32,
    pub end: u32,
    pub message: String,
}

#[derive(Serialize, Default)]
pub struct Summary {
    pub files: usize,
    pub sloc: u64,
    pub functions: usize,
    pub sloc_by_lang: BTreeMap<String, u64>,
    pub slop_score: f64,
    pub fn_sloc_p90: u32,
    pub magic_numbers_per_ksloc: f64,
    pub verbosity: f64,
    pub clone_pct: f64,
    pub erosion: f64,
    pub cog_erosion: f64,
    pub long_fn_share: f64,
    pub cc_p90: u32,
    pub cog_p90: u32,
    pub call_depth_max: u32,
    pub findings_per_ksloc: BTreeMap<String, f64>,
}

fn ratio(a: f64, b: f64) -> f64 {
    if b > 0.0 { a / b + 0.0 } else { 0.0 }
}

fn p90(mut v: Vec<u32>) -> u32 {
    v.sort_unstable();
    v.get(v.len() * 9 / 10).copied().unwrap_or(0)
}

fn mass_share(fns: &[&Function], value: impl Fn(&Function) -> u32) -> f64 {
    let mass = |f: &&Function| value(f) as f64 * (f.sloc as f64).sqrt();
    ratio(fns.iter().filter(|f| value(f) > 10).map(mass).sum(), fns.iter().map(mass).sum())
}

pub fn summarize(files: &[FileFacts], dups: &Duplicates, findings: &[Finding], cfg: &Config) -> Summary {
    let fns: Vec<&Function> = files.iter().flat_map(|f| &f.functions).collect();
    let sloc: u64 = files.iter().map(|f| f.sloc_count as u64).sum();
    let ksloc = sloc as f64 / 1000.0;
    let mut s = Summary { files: files.len(), sloc, functions: fns.len(), ..Default::default() };
    let (mut clone_lines, mut flagged) = (0u64, 0u64);
    for (i, f) in files.iter().enumerate() {
        *s.sloc_by_lang.entry(format!("{:?}", f.lang).to_lowercase()).or_default() += f.sloc_count as u64;
        let mut rule_lines = vec![false; f.sloc.len()];
        for h in f.hits.iter().filter(|h| VERBOSITY_RULES.contains(&h.rule) && cfg.level(h.rule) != Level::Off) {
            rule_lines[h.start as usize..=(h.end as usize).min(f.sloc.len() - 1)].fill(true);
        }
        for (l, _) in f.sloc.iter().enumerate().filter(|(_, code)| **code) {
            let cloned = dups.subtree_lines[i][l] || dups.token_lines[i][l];
            clone_lines += cloned as u64;
            flagged += (cloned || rule_lines[l]) as u64;
        }
    }
    let magic = files.iter().flat_map(|f| &f.hits).filter(|h| h.rule == "magic-number").count();
    s.magic_numbers_per_ksloc = ratio(magic as f64, ksloc);
    s.fn_sloc_p90 = p90(fns.iter().map(|f| f.sloc).collect());
    s.slop_score = (s.fn_sloc_p90 as f64 / 30.0 + s.magic_numbers_per_ksloc.ln_1p() / 40f64.ln_1p()) / 2.0;
    s.verbosity = ratio(flagged as f64, sloc as f64);
    s.clone_pct = ratio(clone_lines as f64, sloc as f64);
    s.erosion = mass_share(&fns, |f| f.cc);
    s.cog_erosion = mass_share(&fns, |f| f.cog);
    let fn_sloc: f64 = fns.iter().map(|f| f.sloc as f64).sum();
    s.long_fn_share = ratio(fns.iter().filter(|f| f.sloc > cfg.limits.function_sloc).map(|f| f.sloc as f64).sum(), fn_sloc);
    s.cc_p90 = p90(fns.iter().map(|f| f.cc).collect());
    s.cog_p90 = p90(fns.iter().map(|f| f.cog).collect());
    s.call_depth_max = fns.iter().map(|f| f.depth).max().unwrap_or(0);
    for f in findings {
        *s.findings_per_ksloc.entry(f.rule.clone()).or_default() += 1.0 / ksloc.max(0.001);
    }
    s
}

pub fn findings(files: &[FileFacts], dups: &Duplicates, chains: &[Chain], cfg: &Config) -> Vec<Finding> {
    let lim = &cfg.limits;
    let mut out = vec![];
    let mut push = |rule: &str, path: &str, start: u32, end: u32, message: String| {
        let level = cfg.level(rule);
        if level != Level::Off {
            out.push(Finding { rule: rule.to_string(), level, path: path.to_string(), start, end, message });
        }
    };
    for f in files {
        let path = f.path.to_string_lossy();
        if f.sloc_count > lim.file_sloc {
            push("large-file", &path, 1, f.sloc.len() as u32, format!("{} SLOC (limit {})", f.sloc_count, lim.file_sloc));
        }
        for u in &f.functions {
            let checks = [
                ("complexity", u.cc, lim.cyclomatic, "cyclomatic complexity", u.start),
                ("cognitive-complexity", u.cog, lim.cognitive, "cognitive complexity", u.start),
                ("long-function", u.sloc, lim.function_sloc, "SLOC", u.start),
                ("deep-nesting", u.nesting, lim.nesting, "nesting depth", u.start),
                ("too-many-params", u.params, lim.params, "parameters", u.start),
                ("if-ladder", u.ladder, lim.if_ladder, "if/else-if branches", u.ladder_line),
                ("complex-condition", u.bool_ops, lim.condition_ops, "boolean operators in one condition", u.bool_line),
            ];
            for (rule, value, limit, what, line) in checks.into_iter().filter(|c| c.1 > c.2) {
                let end = if line == u.start { u.end } else { line };
                push(rule, &path, line, end, format!("`{}`: {value} {what} (limit {limit})", u.name));
            }
        }
        for (u, values) in magic_by_function(f).into_iter().filter(|(_, v)| v.len() as u32 >= lim.magic_numbers) {
            let shown: Vec<_> = values.iter().take(6).map(String::as_str).collect();
            let msg = format!("`{}` uses {} magic numbers ({}…); name them as constants", u.name, values.len(), shown.join(", "));
            push("magic-numbers", &path, u.start, u.end, msg);
        }
        for h in f.hits.iter().filter(|h| h.rule != "magic-number") {
            push(h.rule, &path, h.start, h.end, h.message.clone());
        }
    }
    for c in chains.iter().filter(|c| c.path.len() as u32 > lim.call_depth) {
        let u = &files[c.file].functions[c.function];
        let path = files[c.file].path.to_string_lossy();
        push("deep-call-chain", &path, u.start, u.start, format!("call chain {} functions deep: {}", c.path.len(), c.path.join(" -> ")));
    }
    for d in &dups.found {
        let other = &files[d.other.file];
        let msg = format!("{} lines duplicated ({} copies), e.g. {}:{}", d.span.end - d.span.start + 1, d.copies, other.path.display(), d.other.start);
        push("duplicate-code", &files[d.span.file].path.to_string_lossy(), d.span.start, d.span.end, msg);
    }
    let ignores: BTreeMap<String, &Vec<(u32, Vec<String>)>> = files.iter().map(|f| (f.path.to_string_lossy().into_owned(), &f.ignores)).collect();
    out.retain(|x| {
        !ignores[&x.path].iter().any(|(line, rules)| (x.start == *line || x.start == line + 1) && (rules.is_empty() || rules.contains(&x.rule)))
    });
    out.sort_by(|x, y| (&x.path, x.start, &x.rule).cmp(&(&y.path, y.start, &y.rule)));
    out
}

fn magic_by_function(f: &FileFacts) -> Vec<(&Function, Vec<String>)> {
    let mut by_fn: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for h in f.hits.iter().filter(|h| h.rule == "magic-number") {
        let inner = f.functions.iter().enumerate().filter(|(_, u)| u.start <= h.start && h.start <= u.end).min_by_key(|(_, u)| u.end - u.start);
        if let Some((i, _)) = inner {
            by_fn.entry(i).or_default().push(h.message.clone());
        }
    }
    by_fn.into_iter().map(|(i, v)| (&f.functions[i], v)).collect()
}
