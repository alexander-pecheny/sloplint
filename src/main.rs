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
#[command(about = "Maintainability and slop gate for Python, Rust, Go, JS, TS and Swift")]
struct Cli {
    #[arg(default_value = ".")]
    paths: Vec<PathBuf>,
    #[arg(long, help = "Gate only findings that touch lines staged for commit")]
    staged: bool,
    #[arg(long, value_name = "REV", help = "Gate only findings that touch lines changed since REV")]
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

fn discover(paths: &[PathBuf], cfg: &Config) -> Result<Vec<PathBuf>> {
    let mut overrides = ignore::overrides::OverrideBuilder::new(".");
    for pat in &cfg.exclude {
        overrides.add(&format!("!{pat}"))?;
    }
    let overrides = overrides.build()?;
    let mut out = vec![];
    for root in paths {
        let walk = ignore::WalkBuilder::new(root)
            .overrides(overrides.clone())
            .filter_entry(|e| {
                let name = e.file_name().to_str().unwrap_or("");
                !DEFAULT_EXCLUDES.contains(&name) && !name.ends_with(".docc")
            })
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

fn load(path: &Path, include_tests: bool) -> Option<analyze::FileFacts> {
    let lang = lang::Lang::from_path(path)?;
    let is_test = lang::is_test_path(path);
    if is_test && !include_tests {
        return None;
    }
    let src = std::fs::read_to_string(path).ok()?;
    if analyze::is_generated(path, &src) {
        return None;
    }
    analyze::analyze(path.to_path_buf(), lang, &src).filter(|f| !analyze::is_data(f))
}

fn canonical(changed: git::Changed) -> git::Changed {
    changed.into_iter().filter_map(|(p, r)| Some((std::fs::canonicalize(p).ok()?, r))).collect()
}

fn touches(changed: &git::Changed, f: &report::Finding) -> bool {
    let Ok(abs) = std::fs::canonicalize(&f.path) else { return false };
    changed.get(&abs).is_some_and(|ranges| ranges.iter().any(|&(a, b)| a <= f.end && f.start <= b))
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
    let mut paths = discover(&cli.paths, &cfg)?;
    let changed = if cli.staged || cli.diff.is_some() { Some(canonical(git::changed_lines(cli.diff.as_deref())?)) } else { None };
    let cross_file = ["duplicate-code", "deep-call-chain"].iter().any(|r| cfg.level(r) == Level::Error);
    if let (Some(changed), false) = (&changed, cross_file) {
        paths.retain(|p| std::fs::canonicalize(p).is_ok_and(|p| changed.contains_key(&p)));
    }
    let mut files: Vec<_> = paths.par_iter().filter_map(|p| load(p, cfg.include_tests)).collect();
    let chains = callgraph::chains(&mut files);
    let dups = clones::detect(&files, &cfg.clones);
    let mut findings = report::findings(&files, &dups, &chains, &cfg);
    let summary = report::summarize(&files, &dups, &findings, &cfg);

    if let Some(changed) = &changed {
        findings.retain(|f| touches(changed, f));
    }
    let over_score = cfg.project.max_slop_score.filter(|&l| changed.is_none() && summary.slop_score > l);
    let failed = over_score.is_some() || findings.iter().any(|f| f.level == Level::Error);

    match cli.format {
        Format::Json => println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "summary": summary, "findings": findings }))?),
        Format::Summary => println!("{}", serde_json::to_string_pretty(&summary)?),
        Format::Functions => print_functions(&files),
        Format::Text => print_text(&findings, &summary, over_score, cli.warnings),
    }
    std::process::exit(failed as i32);
}
