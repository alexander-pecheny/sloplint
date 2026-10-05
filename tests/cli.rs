use std::path::Path;
use std::process::Command;

fn sloplint(args: &[&str], dir: &Path) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_sloplint")).args(args).current_dir(dir).output().unwrap();
    (out.status.code().unwrap(), String::from_utf8(out.stdout).unwrap())
}

fn fixtures() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
}

fn errors(json: &str) -> Vec<(String, String)> {
    let v: serde_json::Value = serde_json::from_str(json).unwrap();
    v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["level"] == "error")
        .map(|f| {
            let path = f["path"].as_str().unwrap();
            (path.rsplit('/').next().unwrap().to_string(), f["rule"].as_str().unwrap().to_string())
        })
        .collect()
}

#[test]
fn every_language_trips_each_gate_rule() {
    let (code, out) = sloplint(&["slop", "--include-tests", "--format", "json"], fixtures());
    assert_eq!(code, 1);
    let found = errors(&out);
    for file in ["app.py", "app.go", "app.rs", "app.js", "app.ts", "App.swift"] {
        for rule in ["magic-numbers", "hardcoded-color", "error-only-printed"] {
            assert!(found.contains(&(file.into(), rule.into())), "{file} should trip {rule}: {found:?}");
        }
    }
    assert_eq!(found.len(), 21, "{found:?}");
}

#[test]
fn clean_code_and_ignore_comments_pass() {
    let (code, out) = sloplint(&["clean", "--include-tests", "--format", "json"], fixtures());
    assert_eq!(errors(&out), vec![]);
    assert_eq!(code, 0);
}

#[test]
fn complexity_matches_hand_count() {
    let (_, out) = sloplint(&["complexity.py", "--include-tests", "--format", "functions"], fixtures());
    let rows: Vec<Vec<&str>> = out.lines().skip(1).map(|l| l.split('\t').collect()).collect();
    let ladder = rows.iter().find(|r| r[3] == "ladder").unwrap();
    let nested = rows.iter().find(|r| r[3] == "nested").unwrap();
    assert_eq!((ladder[5], ladder[6], ladder[9]), ("7", "7", "5"), "cc, cognitive, ladder");
    assert_eq!((nested[5], nested[6], nested[7]), ("5", "10", "4"), "cc, cognitive, nesting");
}

#[test]
fn staged_mode_gates_only_changed_lines() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("staged-repo");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| assert!(Command::new("git").args(args).current_dir(&dir).output().unwrap().status.success());
    git(&["init", "-q"]);
    git(&["config", "user.email", "t@t"]);
    git(&["config", "user.name", "t"]);
    std::fs::write(dir.join("old.py"), "def theme():\n    return '#ffffff'\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "init"]);
    std::fs::write(dir.join("new.py"), "def accent():\n    return '#ff0000'\n").unwrap();
    git(&["add", "new.py"]);
    let (code, out) = sloplint(&["--staged", "--format", "json"], &dir);
    assert_eq!(errors(&out), vec![("new.py".into(), "hardcoded-color".into())]);
    assert_eq!(code, 1);
}
