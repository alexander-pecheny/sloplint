use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Diff {
    pub root: PathBuf,
    pub changed: HashMap<PathBuf, Vec<(u32, u32)>>,
    pub source: Source,
}

pub enum Source {
    Worktree,
    Index,
    Rev(String),
}

impl Diff {
    pub fn relative(&self, path: &Path) -> Option<PathBuf> {
        match self.source {
            Source::Worktree => std::fs::canonicalize(path).ok()?.strip_prefix(&self.root).ok().map(Path::to_path_buf),
            _ => Some(path.to_path_buf()),
        }
    }

    pub fn touches(&self, path: &Path, start: u32, end: u32) -> bool {
        let ranges = self.relative(path).and_then(|r| self.changed.get(&r));
        ranges.is_some_and(|ranges| ranges.iter().any(|&(a, b)| a <= end && start <= b))
    }

    pub fn read(&self, rel: &Path) -> Option<String> {
        let spec = match &self.source {
            Source::Worktree => return std::fs::read_to_string(self.root.join(rel)).ok(),
            Source::Index => format!(":{}", rel.display()),
            Source::Rev(rev) => format!("{rev}:{}", rel.display()),
        };
        git(&self.root, &["show", &spec]).ok()
    }

    pub fn tracked(&self) -> Result<Vec<PathBuf>> {
        let out = match &self.source {
            Source::Rev(rev) => git(&self.root, &["ls-tree", "-r", "--name-only", rev])?,
            _ => git(&self.root, &["ls-files"])?,
        };
        Ok(out.lines().map(PathBuf::from).collect())
    }
}

pub fn diff(spec: Option<&str>) -> Result<Diff> {
    let root = PathBuf::from(git(Path::new("."), &["rev-parse", "--show-toplevel"])?.trim());
    let (range, source) = match spec {
        None => ("--cached".to_string(), Source::Index),
        Some(s) => match s.rsplit_once("..") {
            Some((_, right)) => {
                let right = right.trim_start_matches('.');
                (s.to_string(), Source::Rev(if right.is_empty() { "HEAD".into() } else { right.into() }))
            }
            None => (s.to_string(), Source::Worktree),
        },
    };
    let out = git(&root, &["diff", "--unified=0", "--no-color", "--no-ext-diff", "--diff-filter=AMR", &range])?;
    let mut changed: HashMap<PathBuf, Vec<(u32, u32)>> = HashMap::new();
    let mut current: Option<PathBuf> = None;
    for l in out.lines() {
        if let Some(p) = l.strip_prefix("+++ ") {
            current = p.strip_prefix("b/").map(PathBuf::from);
        } else if let (Some(hunk), Some(path)) = (l.strip_prefix("@@ "), &current) {
            let new = hunk.split_whitespace().find(|s| s.starts_with('+')).context("bad hunk")?;
            let mut parts = new[1..].split(',');
            let start: u32 = parts.next().unwrap_or("0").parse()?;
            let len: u32 = parts.next().map_or(Ok(1), str::parse)?;
            if len > 0 {
                changed.entry(path.clone()).or_default().push((start, start + len - 1));
            }
        }
    }
    Ok(Diff { root, changed, source })
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git").args(args).current_dir(dir).output().context("running git")?;
    if !out.status.success() {
        bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8(out.stdout)?)
}
