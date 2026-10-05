mod analyze;
mod callgraph;
mod clones;
mod config;
mod git;
mod lang;
mod report;
mod rules;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use config::{Config, Level};
use rayon::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(about = "Maintainability and slop gate for Python, Rust, Go, JS, TS, Swift and PHP")]
struct Cli {
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
    #[arg(long, help = "Gate only findings that touch lines staged for commit")]
    staged: bool,
    #[arg(long, value_name = "SPEC", help = "Gate only findings that touch lines in `git diff SPEC`: REV, A..B or A...B")]
    diff: Option<String>,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long, help = "Also analyze test files")]
    include_tests: bool,
    #[arg(long, help = "Print warnings as well as errors")]
    warnings: bool,
}

#[derive(Clone, Copy, ValueEnum, PartialEq)]
enum Format {
    Text,
    Json,
    Summary,
    Functions,
}

const DEFAULT_EXCLUDES: &[&str] = &[
    "node_modules", "vendor", "dist", "build", ".build", "target", "third_party", "Pods", ".venv", "venv",
    "__pycache__", "DerivedData", "coverage", ".next", "docs",
];

fn excludes(root: &Path, cfg: &Config) -> Result<ignore::overrides::Override> {
    let mut overrides = ignore::overrides::OverrideBuilder::new(root);
    for pat in &cfg.exclude {
        overrides.add(&format!("!{pat}"))?;
    }
    Ok(overrides.build()?)
}

fn excluded_dir(name: &str) -> bool {
    DEFAULT_EXCLUDES.contains(&name) || name.ends_with(".docc")
}

fn discover(paths: &[PathBuf], cfg: &Config) -> Result<Vec<PathBuf>> {
    let overrides = excludes(Path::new("."), cfg)?;
    let mut out = vec![];
    for root in paths {
        let walk = ignore::WalkBuilder::new(root)
            .overrides(overrides.clone())
            .filter_entry(|e| !excluded_dir(e.file_name().to_str().unwrap_or("")))
            .build();
        for entry in walk {
            let entry = entry?;
            if entry.file_type().is_some_and(|t| t.is_file()) && lang::Lang::from_path(entry.path()).is_some() {
                out.push(entry.into_path());
            }
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn from_git(diff: &git::Diff, paths: &[PathBuf], cfg: &Config, cross_file: bool) -> Result<Vec<PathBuf>> {
    let overrides = excludes(&diff.root, cfg)?;
    let prefixes: Vec<PathBuf> = paths.iter().filter_map(|p| std::fs::canonicalize(p).ok()?.strip_prefix(&diff.root).ok().map(Path::to_path_buf)).collect();
    let candidates = if cross_file { diff.tracked()? } else { diff.changed.keys().cloned().collect() };
    let mut out: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|p| lang::Lang::from_path(p).is_some() && prefixes.iter().any(|pre| p.starts_with(pre)))
        .filter(|p| !p.iter().any(|c| excluded_dir(c.to_str().unwrap_or(""))))
        .filter(|p| !overrides.matched(diff.root.join(p), false).is_ignore())
        .collect();
    out.sort();
    Ok(out)
}

fn load(path: &Path, src: Option<String>, include_tests: bool) -> Option<analyze::FileFacts> {
    let lang = lang::Lang::from_path(path)?;
    if lang::is_test_path(path) && !include_tests {
        return None;
    }
    let src = src?;
    if analyze::is_generated(path, &src) {
        return None;
    }
    analyze::analyze(path.to_path_buf(), lang, &src).filter(|f| !analyze::is_data(f))
}

fn print_functions(files: &[analyze::FileFacts]) {
    println!("path\tline\tend\tname\tsloc\tcc\tcog\tnesting\tparams\tladder\tbool_ops\tdepth");
    for f in files {
        for u in &f.functions {
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                f.path.display(), u.start, u.end, u.name, u.sloc, u.cc, u.cog, u.nesting, u.params, u.ladder, u.bool_ops, u.depth
            );
        }
    }
}

fn print_text(findings: &[report::Finding], s: &report::Summary, over_score: Option<f64>, warnings: bool) {
    for f in findings.iter().filter(|f| warnings || f.level == Level::Error) {
        let tag = if f.level == Level::Error { "error" } else { "warn" };
        println!("{}:{}: {tag}[{}] {}", f.path, f.start, f.rule, f.message);
    }
    println!(
        "\n{} files, {} SLOC, {} functions | slop score {:.2} (p90 function {} SLOC, {:.0} magic numbers/kSLOC) | verbosity {:.3} | erosion {:.3}",
        s.files, s.sloc, s.functions, s.slop_score, s.fn_sloc_p90, s.magic_numbers_per_ksloc, s.verbosity, s.erosion
    );
    if let Some(limit) = over_score {
        println!("error: slop score {:.2} is above the project limit {limit}", s.slop_score);
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut cfg = Config::load(cli.config.as_deref())?;
    cfg.include_tests |= cli.include_tests;
    let diff = if cli.staged || cli.diff.is_some() { Some(git::diff(cli.diff.as_deref())?) } else { None };
    let cross_file = ["duplicate-code", "deep-call-chain"].iter().any(|r| cfg.level(r) == Level::Error);
    let mut files: Vec<_> = match &diff {
        Some(d) if !matches!(d.source, git::Source::Worktree) => {
            from_git(d, &cli.paths, &cfg, cross_file)?.par_iter().filter_map(|p| load(p, d.read(p), cfg.include_tests)).collect()
        }
        _ => {
            let mut paths = discover(&cli.paths, &cfg)?;
            if let (Some(d), false) = (&diff, cross_file) {
                paths.retain(|p| d.relative(p).is_some_and(|r| d.changed.contains_key(&r)));
            }
            paths.par_iter().filter_map(|p| load(p, std::fs::read_to_string(p).ok(), cfg.include_tests)).collect()
        }
    };
    let chains = callgraph::chains(&mut files);
    let dups = clones::detect(&files, &cfg.clones);
    let mut findings = report::findings(&files, &dups, &chains, &cfg);
    let summary = report::summarize(&files, &dups, &findings, &cfg);

    if let Some(d) = &diff {
        findings.retain(|f| d.touches(Path::new(&f.path), f.start, f.end));
    }
    let over_score = cfg.project.max_slop_score.filter(|&l| diff.is_none() && summary.slop_score > l);
    let failed = over_score.is_some() || findings.iter().any(|f| f.level == Level::Error);

    match cli.format {
        Format::Json => println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "summary": summary, "findings": findings }))?),
        Format::Summary => println!("{}", serde_json::to_string_pretty(&summary)?),
        Format::Functions => print_functions(&files),
        Format::Text => print_text(&findings, &summary, over_score, cli.warnings),
    }
    std::process::exit(failed as i32);
}
