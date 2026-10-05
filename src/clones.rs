use crate::analyze::FileFacts;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Deserialize, Serialize, Clone)]
#[serde(default, rename_all = "kebab-case")]
pub struct CloneConfig {
    pub min_tokens: usize,
    pub min_lines: u32,
    pub min_distinct_tokens: usize,
    pub subtree_min_sloc: u32,
}

impl Default for CloneConfig {
    fn default() -> Self {
        CloneConfig { min_tokens: 60, min_lines: 6, min_distinct_tokens: 12, subtree_min_sloc: 5 }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Span {
    pub file: usize,
    pub start: u32,
    pub end: u32,
}

pub struct Duplicate {
    pub span: Span,
    pub other: Span,
    pub copies: usize,
}

pub struct Duplicates {
    pub found: Vec<Duplicate>,
    pub subtree_lines: Vec<Vec<bool>>,
    pub token_lines: Vec<Vec<bool>>,
}

pub fn detect(files: &[FileFacts], cfg: &CloneConfig) -> Duplicates {
    let mut found = vec![];
    let subtree_lines = subtree_clones(files, cfg, &mut found);
    let token_lines = token_clones(files, cfg, &mut found);
    Duplicates { found, subtree_lines, token_lines }
}

fn masks(files: &[FileFacts]) -> Vec<Vec<bool>> {
    files.iter().map(|f| vec![false; f.sloc.len()]).collect()
}

fn mark(mask: &mut [bool], start: u32, end: u32) {
    let end = (end as usize).min(mask.len().saturating_sub(1));
    mask[start as usize..=end].fill(true);
}

fn subtree_clones(files: &[FileFacts], cfg: &CloneConfig, found: &mut Vec<Duplicate>) -> Vec<Vec<bool>> {
    let mut all: Vec<(u64, Span)> = files
        .iter()
        .enumerate()
        .flat_map(|(i, f)| {
            f.subtrees
                .iter()
                .filter(|s| s.sloc >= cfg.subtree_min_sloc)
                .map(move |s| (s.hash, Span { file: i, start: s.start, end: s.end }))
        })
        .collect();
    all.sort_by_key(|(h, s)| (*h, s.file, s.start));
    let mut groups: Vec<Vec<Span>> = all
        .chunk_by(|a, b| a.0 == b.0)
        .filter(|g| g.len() > 1)
        .map(|g| g.iter().map(|x| x.1).collect())
        .collect();
    groups.sort_by_key(|g| std::cmp::Reverse(g.iter().map(|s| s.end - s.start).sum::<u32>()));
    let mut lines = masks(files);
    for g in groups {
        if g.iter().all(|s| lines[s.file][s.start as usize..=s.end as usize].iter().all(|&l| l)) {
            continue;
        }
        for (i, s) in g.iter().enumerate() {
            mark(&mut lines[s.file], s.start, s.end);
            found.push(Duplicate { span: *s, other: g[(i + 1) % g.len()], copies: g.len() });
        }
    }
    lines
}

fn token_clones(files: &[FileFacts], cfg: &CloneConfig, found: &mut Vec<Duplicate>) -> Vec<Vec<bool>> {
    const BASE: u64 = 0x100000001b3;
    let w = cfg.min_tokens;
    let top = BASE.wrapping_pow(w as u32 - 1);
    let mut windows: Vec<(u64, u32, u32)> = vec![];
    for (fi, f) in files.iter().enumerate() {
        if f.tokens.len() < w {
            continue;
        }
        let mut h: u64 = 0;
        for (i, t) in f.tokens.iter().enumerate() {
            if i >= w {
                h = h.wrapping_sub(f.tokens[i - w].hash.wrapping_mul(top));
            }
            h = h.wrapping_mul(BASE).wrapping_add(t.hash);
            if i + 1 >= w {
                windows.push((h, fi as u32, (i + 1 - w) as u32));
            }
        }
    }
    windows.sort_unstable();
    let mut covered: Vec<Vec<Option<(u32, u32)>>> = files.iter().map(|f| vec![None; f.tokens.len()]).collect();
    for g in windows.chunk_by(|a, b| a.0 == b.0).filter(|g| g.len() > 1) {
        let (_, f0, p0) = g[0];
        let toks = &files[f0 as usize].tokens[p0 as usize..p0 as usize + w];
        if toks.last().unwrap().line - toks[0].line + 1 < cfg.min_lines
            || toks.iter().map(|t| t.hash).collect::<HashSet<_>>().len() < cfg.min_distinct_tokens
            || toks.iter().filter(|t| t.code).count() * 10 < w * 4
        {
            continue;
        }
        for &(_, f, p) in g {
            let partner = g.iter().find(|&&(_, of, op)| of != f || op.abs_diff(p) >= w as u32);
            let Some(&(_, of, op)) = partner else { continue };
            for slot in &mut covered[f as usize][p as usize..p as usize + w] {
                slot.get_or_insert((of, op));
            }
        }
    }
    let mut lines = masks(files);
    for (fi, cov) in covered.iter().enumerate() {
        let toks = &files[fi].tokens;
        let mut i = 0;
        while i < cov.len() {
            let Some((of, op)) = cov[i] else {
                i += 1;
                continue;
            };
            let start = i;
            while i < cov.len() && cov[i].is_some() {
                i += 1;
            }
            let (a, b) = (toks[start].line, toks[i - 1].line);
            mark(&mut lines[fi], a, b);
            let other_line = files[of as usize].tokens[op as usize].line;
            let other = Span { file: of as usize, start: other_line, end: other_line + (b - a) };
            found.push(Duplicate { span: Span { file: fi, start: a, end: b }, other, copies: 2 });
        }
    }
    lines
}
