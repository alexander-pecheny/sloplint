use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

pub type Changed = HashMap<PathBuf, Vec<(u32, u32)>>;

pub fn changed_lines(rev: Option<&str>) -> Result<Changed> {
    let root = git(&["rev-parse", "--show-toplevel"])?;
    let root = PathBuf::from(root.trim());
    let mut args = vec!["diff", "--unified=0", "--no-color", "--no-ext-diff", "--diff-filter=AMR"];
    match rev {
        Some(r) => args.push(r),
        None => args.push("--cached"),
    }
    let mut changed = Changed::new();
    let mut current: Option<PathBuf> = None;
    for l in git(&args)?.lines() {
        if let Some(p) = l.strip_prefix("+++ ") {
            current = p.strip_prefix("b/").map(|p| root.join(p));
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
    Ok(changed)
}

fn git(args: &[&str]) -> Result<String> {
    let out = Command::new("git").args(args).output().context("running git")?;
    if !out.status.success() {
        bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr));
    }
    Ok(String::from_utf8(out.stdout)?)
}
