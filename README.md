# sloplint

sloplint is a pre-commit gate for Python, Rust, Go, JavaScript, TypeScript, Swift and PHP. It parses code with tree-sitter and fails a commit only on patterns that, in two validation corpora, appeared about 9–55 times more often in vibe-coded repositories than in respected ones. Classic maintainability metrics are computed too, but they do not separate good code from bad, so they only warn.

It started as a port of [scb-check](https://github.com/gabeorlanski/scb-check), the scorer behind [SlopCodeBench](https://github.com/SprocketLab/slop-code-bench). It reports the same verbosity and erosion scores for every supported language, where scb-check's rules cover mostly Python. On the 49 repositories both tools could score, its verbosity correlates with scb-check's at 0.73 and its erosion at 0.80.

## Usage

```sh
cargo install --path .
sloplint                    # whole tree: errors, then a summary line
sloplint --staged           # only findings that touch staged lines, read from the index
sloplint --diff origin/main # only findings on lines changed between a revision and the working tree
sloplint --diff A..B        # only findings on lines changed between two commits, read from B
sloplint --diff main...HEAD # the same for a branch since it forked from main
sloplint --warnings         # also print warnings
sloplint --exclude gen --exclude '*.pb.ts'  # skip files or directories, gitignore-style
sloplint --format json      # summary and findings
sloplint --format functions # per-function metrics as TSV
```

`--diff` takes anything `git diff` accepts. With a range, sloplint reads the files from its right side, so the working tree does not matter. The exit code is 1 when any error-level finding remains. Test files are skipped unless `--include-tests` is set. So are generated and minified files, data tables, `docs/` and the usual vendor and build directories.

As a [pre-commit](https://pre-commit.com) hook:

```yaml
- repo: https://code.pecheny.me/pecheny/sloplint
  rev: main
  hooks:
    - id: sloplint
```

Or as a plain git hook: `printf '#!/bin/sh\nexec sloplint --staged\n' > .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit`.

To silence a finding, put `sloplint: ignore[rule-id]` in a comment on the same line or the line above. A comment on the line above covers the whole statement or declaration that starts there, so one comment above a palette or a function covers every line of it. A bare `sloplint: ignore` silences every rule. Configuration lives in `sloplint.toml`; see `sloplint.example.toml`.

## Rules

| Rule | Level | What it flags |
|---|---|---|
| `magic-numbers` | error | A function with 8 or more unnamed numeric literals in call arguments, arithmetic, collections or assignments |
| `hardcoded-color` | error | A `#rrggbb` or `rgb(...)` string literal |
| `error-only-printed` | error | A `catch`/`except`, Go `if err != nil` or Rust `Err(e) =>` branch that only prints the error. Not flagged where the error has nowhere to go: a Go function with no `error` result that returns at once or scopes the failed call to the `if` (but `main` and HTTP handlers that return are flagged), or a TS/JS callback or `void` function |
| `debug-print` | warn | `console.log`, `print`, `fmt.Println`, `eprintln!`, `NSLog`, `var_dump` and similar. Rust's `println!` is left alone, because CLIs use it for output |
| `decorative-unicode` | warn | Emoji, arrows, box drawing or check marks in strings and comments |
| `nested-ternary`, `hardcoded-delay`, `broad-except`, `empty-catch` | warn | As named |
| `identical-branches`, `duplicate-condition` | warn | Branches that cannot differ, or conditions that can never be reached |
| `complexity`, `cognitive-complexity`, `long-function`, `deep-nesting`, `too-many-params`, `if-ladder`, `complex-condition`, `large-file`, `duplicate-code` | warn | Classic limits, configurable in `[limits]` |
| `deep-call-chain`, `return-staircase` | off | Longest intra-project call chain; returns cascading down several indentation levels |

Cyclomatic complexity follows McCabe. Cognitive complexity follows SonarSource: `else if` costs +1 without a nesting penalty, and closures add nesting. Duplicate code is found two ways. The first, as in scb-check, hashes syntax subtrees with identifiers renamed. The second, as in CPD, matches 60-token windows.

## Validation

Rules were chosen on one corpus and then checked, unchanged, on a second one.

The first corpus has 79 repositories: 36 respected projects and 43 that their authors describe as vibe-coded or 100% AI-generated, with 5–8 per language. The good side includes CPython's stdlib, the Go stdlib, ripgrep, serde, zod, hono, swift-collections and Alamofire. The bad side is hobby repositories with READMEs such as "every line of code was written by Claude Code".

The held-out corpus has 58 repositories by different authors. Its good side is mostly applications: Home Assistant, Sentry, helix, alacritty, lazygit, gitea, excalidraw, VS Code, IceCubesApp, NetNewsWire, Laravel and Nextcloud. It also adds PHP, which played no part in choosing the rules.

Precision is a rule's hit rate per thousand SLOC in bad repos, divided by the combined rate in bad and good repos. Every repository is weighted equally.

| Rule | Precision, first | Precision, held-out | Bad repos hit, held-out | Good repos hit, held-out | Hits per kSLOC in good code, held-out |
|---|---|---|---|---|---|
| `magic-numbers` | 0.97 | 0.87 | 26/29 | 26/29 | 0.19 |
| `hardcoded-color` | 1.00 | 0.92 | 17/29 | 5/29 | 0.12 |
| `error-only-printed` | 0.96 | 0.91 | 10/29 | 8/29 | 0.03 |
| all three together | 0.98 | 0.90 | 26/29 | 28/29 | 0.35 |
| `debug-print` | 0.83 | 0.88 | 21/29 | 17/29 | 0.56 |
| `long-function` (>80 SLOC) | 0.66 | 0.52 | 27/29 | 27/29 | 0.88 |
| `complexity` (CC >15) | 0.60 | 0.50 | 28/29 | 29/29 | 1.77 |
| `duplicate-code` | 0.48 | 0.61 | 28/29 | 29/29 | 7.4 |

On held-out code the gate fires about once per 2,900 lines of good application code, and 9 times as often in vibe-coded code. One repository, excalidraw, produces most of the good-code colour and printed-error hits; without it the gate's precision is 0.92. The remaining good-code hits are mostly numeric domain code such as terminal emulators, colour utilities and animation curves. That is where an ignore comment or a higher `magic-numbers` limit belongs.

Several plausible signals did not separate the two groups, so they only warn or were dropped. Duplication, cyclomatic and cognitive complexity at any threshold, nesting depth, parameter count, if-ladders, call-chain depth, return staircases, `any` types, lint suppressions, commented-out code, boolean-return idioms and "placeholder" comments all fall in this group. Respected code contains as much duplication and as many complex functions as vibe-coded code.

At the repository level, the summary's `slop_score` averages p90 function length (scaled by 30 SLOC) with log magic-number density. It ranks a random bad repository above a random good one 97% of the time on the first corpus and 71% on the held-out one. Function length lost most of its power once the good side became applications rather than libraries. Magic-number density alone keeps 83%. For comparison, scb-check 0.2.0's own verbosity scores 58% and its erosion 64% on the 49 Python, Rust, JS and TS repositories of the first corpus it could finish. It timed out on fastify and zod.

Known gaps: the Swift grammar fails on about 7% of files from newer Swift (`~Copyable`, `borrowing`), and files more than 10% unparsed are skipped. Every per-language figure rests on 4–8 repositories per side.
