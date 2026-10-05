use crate::lang::{Lang, Spec};
use crate::rules;
use serde::Serialize;
use std::path::PathBuf;
use tree_sitter::{Node, Parser};
use xxhash_rust::xxh3::{Xxh3, xxh3_64};

#[derive(Serialize, Clone)]
pub struct Function {
    pub name: String,
    pub start: u32,
    pub end: u32,
    pub sloc: u32,
    pub cc: u32,
    pub cog: u32,
    pub nesting: u32,
    pub params: u32,
    pub ladder: u32,
    pub ladder_line: u32,
    pub bool_ops: u32,
    pub bool_line: u32,
    pub depth: u32,
    #[serde(skip)]
    pub calls: Vec<String>,
    #[serde(skip)]
    parent: Option<usize>,
}

#[derive(Serialize, Clone)]
pub struct Hit {
    pub rule: &'static str,
    pub start: u32,
    pub end: u32,
    pub message: String,
}

pub struct Token {
    pub hash: u64,
    pub line: u32,
    pub code: bool,
}

pub struct Subtree {
    pub hash: u64,
    pub start: u32,
    pub end: u32,
    pub sloc: u32,
}

pub struct FileFacts {
    pub path: PathBuf,
    pub lang: Lang,
    pub sloc: Vec<bool>,
    pub sloc_count: u32,
    pub functions: Vec<Function>,
    pub tokens: Vec<Token>,
    pub subtrees: Vec<Subtree>,
    pub hits: Vec<Hit>,
    pub ignores: Vec<(u32, Vec<String>)>,
}

pub fn is_data(f: &FileFacts) -> bool {
    f.tokens.len() > 200 && f.tokens.iter().filter(|t| t.code).count() * 100 < f.tokens.len() * 35
}

pub fn is_generated(path: &std::path::Path, src: &str) -> bool {
    let name = path.to_string_lossy();
    let head = &src[..src.floor_char_boundary(2048)];
    [".d.ts", "_pb2.py", ".pb.go"].iter().any(|s| name.ends_with(s))
        || name.contains(".min.")
        || head.contains("DO NOT EDIT")
        || head.contains("@generated")
        || src.len() / src.lines().count().max(1) > 200
}

pub fn analyze(path: PathBuf, lang: Lang, src: &str) -> Option<FileFacts> {
    let mut parser = Parser::new();
    parser.set_language(&lang.grammar()).ok()?;
    let tree = parser.parse(src, None)?;
    let root = tree.root_node();
    if root.has_error() && error_bytes(root) * 10 > src.len() {
        return None;
    }
    let spec = lang.spec();
    let line_count = src.lines().count() + 1;

    let mut comment_mask = vec![false; src.len()];
    mark_comments(root, spec, lang, &mut comment_mask);
    let sloc = sloc_lines(src, &comment_mask, line_count);
    let sloc_count = sloc.iter().filter(|&&b| b).count() as u32;

    let mut ctx = FileCtx { spec, lang, src, sloc: &sloc, functions: vec![], tokens: vec![], subtrees: vec![] };
    ctx.scan(root, None);
    ctx.collect_tokens(root, false);
    ctx.collect_subtrees(root);
    let FileCtx { mut functions, tokens, subtrees, .. } = ctx;
    let mut nested = vec![0; functions.len()];
    for f in &functions {
        if let Some(p) = f.parent {
            nested[p] += f.sloc;
        }
    }
    for (f, n) in functions.iter_mut().zip(nested) {
        f.sloc = f.sloc.saturating_sub(n);
    }
    let hits = rules::run(root, lang, src);
    Some(FileFacts {
        path,
        lang,
        sloc_count,
        sloc,
        functions,
        tokens,
        subtrees,
        hits,
        ignores: ignores(src),
    })
}

pub fn line(n: Node) -> u32 {
    n.start_position().row as u32 + 1
}

pub fn end_line(n: Node) -> u32 {
    n.end_position().row as u32 + 1
}

pub fn text<'a>(n: Node, src: &'a str) -> &'a str {
    &src[n.byte_range()]
}

pub fn children<'t>(n: Node<'t>) -> Vec<Node<'t>> {
    let mut c = n.walk();
    n.children(&mut c).collect()
}

pub fn named_children<'t>(n: Node<'t>) -> Vec<Node<'t>> {
    let mut c = n.walk();
    n.named_children(&mut c).collect()
}

pub fn is_docstring(n: Node) -> bool {
    n.kind() == "expression_statement" && n.named_child_count() == 1 && n.named_child(0).is_some_and(|c| c.kind() == "string")
}

fn error_bytes(n: Node) -> usize {
    if n.is_error() || n.is_missing() {
        return n.byte_range().len();
    }
    if !n.has_error() {
        return 0;
    }
    children(n).into_iter().map(error_bytes).sum()
}

fn ignores(src: &str) -> Vec<(u32, Vec<String>)> {
    src.lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let rest = &l[l.find("sloplint: ignore")? + "sloplint: ignore".len()..];
            let rules = rest.strip_prefix('[').and_then(|r| r.split(']').next()).map_or(vec![], |r| r.split(',').map(|x| x.trim().to_string()).collect());
            Some((i as u32 + 1, rules))
        })
        .collect()
}

fn mark_comments(n: Node, spec: &Spec, lang: Lang, mask: &mut [bool]) {
    if spec.comments.contains(&n.kind()) || (lang == Lang::Python && is_docstring(n)) {
        mask[n.byte_range()].fill(true);
        return;
    }
    for c in children(n) {
        mark_comments(c, spec, lang, mask);
    }
}

fn sloc_lines(src: &str, comment_mask: &[bool], line_count: usize) -> Vec<bool> {
    let mut sloc = vec![false; line_count + 1];
    let mut line = 1;
    for (i, ch) in src.char_indices() {
        if ch == '\n' {
            line += 1;
        } else if !comment_mask[i] && !ch.is_whitespace() && !"{}[]();,:".contains(ch) {
            sloc[line] = true;
        }
    }
    sloc
}

struct FileCtx<'a> {
    spec: &'static Spec,
    lang: Lang,
    src: &'a str,
    sloc: &'a [bool],
    functions: Vec<Function>,
    tokens: Vec<Token>,
    subtrees: Vec<Subtree>,
}

impl<'a> FileCtx<'a> {
    fn sloc_in(&self, start: u32, end: u32) -> u32 {
        (start..=end).filter(|&l| self.sloc.get(l as usize) == Some(&true)).count() as u32
    }

    fn is_unit(&self, n: Node, enclosed: bool) -> bool {
        let k = n.kind();
        if self.spec.functions.contains(&k) {
            return k == "computed_property" || n.child_by_field_name("body").is_some();
        }
        self.spec.lambdas.contains(&k) && (!enclosed || (self.lang.is_js_like() && js_assigned_name(n, self.src).is_some()))
    }

    fn scan(&mut self, n: Node, parent: Option<usize>) {
        let mut parent = parent;
        if self.is_unit(n, parent.is_some()) {
            let mut m = Metrics { file: self, root: n, cc: 1, cog: 0, nesting: 0, ladder: (0, 0), bool_ops: (0, 0), calls: vec![] };
            for c in children(n) {
                m.visit(c, 0, 0);
            }
            let f = Function {
                name: self.unit_name(n).split_whitespace().collect::<Vec<_>>().join(" ").chars().take(80).collect(),
                start: line(n),
                end: end_line(n),
                sloc: self.sloc_in(line(n), end_line(n)),
                cc: m.cc,
                cog: m.cog,
                nesting: m.nesting,
                params: count_params(n, self.lang, self.src),
                ladder: m.ladder.0,
                ladder_line: m.ladder.1,
                bool_ops: m.bool_ops.0,
                bool_line: m.bool_ops.1,
                depth: 0,
                parent,
                calls: std::mem::take(&mut m.calls),
            };
            self.functions.push(f);
            parent = Some(self.functions.len() - 1);
        }
        for c in children(n) {
            self.scan(c, parent);
        }
    }

    fn unit_name(&self, n: Node) -> String {
        if let Some(name) = n.parent().and_then(|p| p.child_by_field_name("name")).filter(|_| n.kind() == "computed_property") {
            return text(name, self.src).to_string();
        }
        if let Some(name) = n.child_by_field_name("name") {
            return text(name, self.src).to_string();
        }
        js_assigned_name(n, self.src).unwrap_or("<anonymous>").to_string()
    }

    fn collect_tokens(&mut self, n: Node, in_literal: bool) {
        let k = n.kind();
        if self.spec.comments.contains(&k) || self.spec.imports.contains(&k) || (self.lang == Lang::Python && is_docstring(n)) {
            return;
        }
        let in_literal = in_literal || self.spec.literals.contains(&k);
        if n.child_count() == 0 {
            let t = text(n, self.src).trim();
            if !t.is_empty() {
                let code = !in_literal && !t.chars().all(|c| ",;:()[]{}".contains(c));
                self.tokens.push(Token { hash: xxh3_64(t.as_bytes()), line: line(n), code });
            }
            return;
        }
        for c in children(n) {
            self.collect_tokens(c, in_literal);
        }
    }

    fn collect_subtrees(&mut self, n: Node) {
        if self.spec.clone_roots.contains(&n.kind()) && self.body_statements(n) >= 2 {
            let mut h = Xxh3::new();
            let mut names = Vec::new();
            self.hash_node(n, &mut h, &mut names);
            self.subtrees.push(Subtree {
                hash: h.digest(),
                start: line(n),
                end: end_line(n),
                sloc: self.sloc_in(line(n), end_line(n)),
            });
        }
        for c in children(n) {
            self.collect_subtrees(c);
        }
    }

    fn body_statements(&self, n: Node) -> u32 {
        let spec = self.spec;
        let mut best = 0;
        for (i, c) in children(n).into_iter().enumerate() {
            let field = n.field_name_for_child(i as u32);
            let k = c.kind();
            if spec.blocks.contains(&k) || matches!(field, Some("body" | "consequence")) || k == "function_body" {
                best = best.max(self.count_statements(c));
            } else if spec.elses.contains(&k) || spec.elifs.contains(&k) || spec.catches.contains(&k) || k == "finally_clause" {
                best = best.max(self.body_statements(c));
            }
        }
        best
    }

    fn count_statements(&self, container: Node) -> u32 {
        named_children(container)
            .into_iter()
            .map(|c| {
                let k = c.kind();
                if self.spec.comments.contains(&k) || (self.lang == Lang::Python && is_docstring(c)) {
                    0
                } else if self.spec.blocks.contains(&k) || k == "function_body" {
                    self.count_statements(c)
                } else {
                    1
                }
            })
            .sum()
    }

    fn hash_node(&self, n: Node, h: &mut Xxh3, names: &mut Vec<&'a str>) {
        let k = n.kind();
        let spec = self.spec;
        if spec.identifiers.contains(&k) {
            let t = &self.src[n.byte_range()];
            let idx = names.iter().position(|x| *x == t).unwrap_or_else(|| {
                names.push(t);
                names.len() - 1
            });
            h.update(&[1]);
            h.update(&(idx as u32).to_le_bytes());
            return;
        }
        h.update(&n.kind_id().to_le_bytes());
        if spec.literals.contains(&k) || n.child_count() == 0 {
            return;
        }
        h.update(b"(");
        for c in children(n) {
            if !(spec.comments.contains(&c.kind()) || self.lang == Lang::Python && is_docstring(c)) {
                self.hash_node(c, h, names);
            }
        }
        h.update(b")");
    }
}

fn js_assigned_name<'a>(n: Node, src: &'a str) -> Option<&'a str> {
    let p = n.parent()?;
    let target = match p.kind() {
        "variable_declarator" => p.child_by_field_name("name"),
        "pair" => p.child_by_field_name("key"),
        "public_field_definition" | "field_definition" => {
            p.child_by_field_name("name").or_else(|| p.child_by_field_name("property"))
        }
        "assignment_expression" => p.child_by_field_name("left"),
        _ => None,
    }?;
    Some(text(target, src))
}

fn count_params(n: Node, lang: Lang, src: &str) -> u32 {
    if lang == Lang::Swift {
        return named_children(n).iter().filter(|c| c.kind() == "parameter").count() as u32;
    }
    let Some(list) = n.child_by_field_name("parameters") else {
        return n.child_by_field_name("parameter").is_some() as u32;
    };
    named_children(list)
        .into_iter()
        .map(|c| match c.kind() {
            "comment" | "line_comment" | "block_comment" | "self_parameter" | "attribute_item" => 0,
            "keyword_separator" | "positional_separator" => 0,
            "identifier" if lang == Lang::Python && matches!(text(c, src), "self" | "cls") => 0,
            "parameter_declaration" => {
                let mut cur = c.walk();
                c.children_by_field_name("name", &mut cur).count().max(1) as u32
            }
            _ => 1,
        })
        .sum()
}

struct Metrics<'f, 'a, 't> {
    file: &'f FileCtx<'a>,
    root: Node<'t>,
    cc: u32,
    cog: u32,
    nesting: u32,
    ladder: (u32, u32),
    bool_ops: (u32, u32),
    calls: Vec<String>,
}

impl<'t> Metrics<'_, '_, 't> {
    fn bool_op(&self, n: Node) -> Option<&'static str> {
        let spec = self.file.spec;
        if !spec.bool_nodes.contains(&n.kind()) {
            return None;
        }
        let kids = children(n);
        spec.bool_ops.iter().copied().find(|op| kids.iter().any(|c| c.kind() == *op))
    }

    fn count_bool_ops(&self, n: Node) -> u32 {
        let own = self.bool_op(n).is_some() as u32;
        own + children(n).into_iter().map(|c| self.count_bool_ops(c)).sum::<u32>()
    }

    fn check_condition(&mut self, n: Node) {
        if let Some(c) = n.child_by_field_name("condition") {
            let ops = self.count_bool_ops(c);
            if ops > self.bool_ops.0 {
                self.bool_ops = (ops, line(c));
            }
        }
    }

    fn nest(&mut self, n: Node<'t>, nesting: u32, depth: u32) {
        for c in children(n) {
            self.inside(c, nesting, depth);
        }
    }

    fn inside(&mut self, c: Node<'t>, nesting: u32, depth: u32) {
        self.nesting = self.nesting.max(depth + 1);
        self.visit(c, nesting + 1, depth + 1);
    }

    fn visit(&mut self, n: Node<'t>, nesting: u32, depth: u32) {
        if n != self.root && self.file.is_unit(n, true) {
            return;
        }
        let spec = self.file.spec;
        let k = n.kind();
        let is_call = matches!(k, "call" | "call_expression" | "function_call_expression" | "member_call_expression" | "scoped_call_expression");
        if is_call && let Some(name) = callee_name(n, self.file.src) {
            self.calls.push(name);
        }
        if spec.ifs.contains(&k) {
            self.cc += 1;
            self.cog += 1 + nesting;
            let mut len = 1;
            self.if_chain(n, nesting, depth, &mut len);
            if len > self.ladder.0 {
                self.ladder = (len, line(n));
            }
            return;
        }
        let is_switch = spec.switches.contains(&k);
        if is_switch || spec.loops.contains(&k) || spec.catches.contains(&k) || spec.ternaries.contains(&k) {
            self.cc += !is_switch as u32;
            self.cog += 1 + nesting;
            self.check_condition(n);
            if spec.ternaries.contains(&k) {
                for c in children(n) {
                    self.visit(c, nesting + 1, depth);
                }
            } else {
                self.nest(n, nesting, depth);
            }
            return;
        }
        if spec.cases.contains(&k) && !is_default_case(n, self.file.src) {
            self.cc += 1;
        }
        if let Some(op) = self.bool_op(n) {
            self.cc += 1;
            let continues = n.parent().is_some_and(|p| self.bool_op(p) == Some(op));
            self.cog += !continues as u32;
        }
        let inner = nesting + spec.lambdas.contains(&k) as u32;
        for c in children(n) {
            self.visit(c, inner, depth);
        }
    }

    fn if_chain(&mut self, n: Node<'t>, nesting: u32, depth: u32, len: &mut u32) {
        let spec = self.file.spec;
        let guard = n.kind() == "guard_statement";
        self.check_condition(n);
        let mut after_else = false;
        for c in children(n) {
            let k = c.kind();
            if k == "else" {
                after_else = true;
                continue;
            }
            if !c.is_named() {
                continue;
            }
            let else_if = if spec.elses.contains(&k) {
                named_children(c).into_iter().find(|x| spec.ifs.contains(&x.kind()))
            } else if after_else && !guard && spec.ifs.contains(&k) {
                Some(c)
            } else {
                None
            };
            if let Some(next) = else_if {
                self.cc += 1;
                self.cog += 1;
                *len += 1;
                self.if_chain(next, nesting, depth, len);
            } else if spec.elifs.contains(&k) {
                self.cc += 1;
                self.cog += 1;
                *len += 1;
                self.check_condition(c);
                self.inside(c, nesting, depth);
            } else {
                if spec.elses.contains(&k) || (after_else && !guard) {
                    self.cog += 1;
                }
                self.inside(c, nesting, depth);
            }
        }
    }
}

fn callee_name(n: Node, src: &str) -> Option<String> {
    let callee = n.child_by_field_name("function").or_else(|| n.child_by_field_name("name")).or_else(|| n.named_child(0))?;
    let t = text(callee, src);
    let t = t.split(['(', '<', '[']).next()?;
    let last = t.rsplit(|c: char| !(c.is_alphanumeric() || c == '_')).find(|s| !s.is_empty())?;
    Some(last.to_string())
}

fn is_default_case(n: Node, src: &str) -> bool {
    let t = text(n, src).trim_start();
    t.starts_with("default") || t.starts_with("_ ") || t.starts_with("_=") || t.starts_with("case _:") || t.starts_with("case _ ")
}
