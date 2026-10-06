use crate::analyze::{Hit, children, end_line, is_docstring, line, named_children, text};
use crate::lang::{Lang, Spec};
use tree_sitter::Node;

pub const VERBOSITY_RULES: &[&str] = &["identical-branches", "duplicate-condition", "empty-catch", "nested-ternary"];

const DEBUG_PRINTS: &[&str] = &[
    "console.log", "console.debug", "eprintln!", "dbg!", "fmt.Println", "fmt.Printf", "NSLog", "debugPrint", "print",
    "var_dump", "print_r", "dd", "dump",
];
const ERROR_PRINTS: &[&str] = &[
    "print(", "console.log", "console.error", "console.warn", "fmt.Print", "log.Print", "println(", "NSLog", "eprintln!", "println!", "debugPrint(",
    "echo ", "var_dump(", "print_r(",
];
const SLEEPS: &[&str] = &["sleep", "setTimeout", "setInterval", "asyncAfter", "usleep"];
const NUMBER_KINDS: &[&str] = &["integer", "float", "integer_literal", "float_literal", "int_literal", "number", "real_literal"];
const CONST_CONTEXTS: &[&str] = &[
    "const_item", "static_item", "const_declaration", "const_spec", "enum_item", "enum_declaration", "enum_assignment",
];

pub fn run(root: Node, lang: Lang, src: &str) -> Vec<Hit> {
    let mut r = Rules { lang, spec: lang.spec(), src, hits: vec![] };
    r.walk(root);
    r.hits
}

struct Rules<'a> {
    lang: Lang,
    spec: &'static Spec,
    src: &'a str,
    hits: Vec<Hit>,
}

impl<'t> Rules<'_> {
    fn hit(&mut self, rule: &'static str, n: Node, message: String) {
        self.hits.push(Hit { rule, start: line(n), end: end_line(n), message });
    }

    fn walk(&mut self, n: Node<'t>) {
        let k = n.kind();
        let spec = self.spec;
        let t = text(n, self.src);
        if spec.comments.contains(&k) || spec.strings.contains(&k) {
            self.literal_or_comment(n, t, spec.strings.contains(&k));
        } else if NUMBER_KINDS.contains(&k) && !matches!(t, "0" | "1" | "2" | "0.0" | "1.0") && !self.in_const(n) {
            if matches!(number_context(n), "call argument" | "arithmetic" | "collection" | "assignment") {
                self.hit("magic-number", n, t.to_string());
            }
        } else if matches!(k, "call" | "call_expression" | "macro_invocation" | "function_call_expression") {
            self.call(n, k);
        } else if matches!(k, "if_statement" | "if_expression") {
            if !n.parent().is_some_and(|p| p.kind() == "else_clause" || self.is_if(p)) {
                self.duplicate_conditions(n);
            }
            let (cons, alt, has_elif) = self.if_parts(n);
            if let (Some(c), Some(e), false) = (cons, alt.filter(|a| !self.is_if(*a)), has_elif) {
                let ct = squash(text(c, self.src));
                if ct.len() > 2 && ct == squash(text(e, self.src)) {
                    self.hit("identical-branches", n, "if and else branches are identical".into());
                }
            }
        } else if spec.ternaries.contains(&k) && k != "if_clause" {
            self.ternary(n);
        } else if spec.catches.contains(&k) {
            self.catch(n);
        }
        if spec.functions.contains(&k) || spec.lambdas.contains(&k) {
            let body = n.child_by_field_name("body").or_else(|| named_children(n).into_iter().find(|c| c.kind() == "function_body"));
            let levels = body.map_or(0, |b| self.staircase(b));
            if levels >= 3 {
                self.hit("return-staircase", n, format!("returns cascade down {levels} indentation levels; flatten with early returns"));
            }
        }
        self.error_only_printed(n, k);
        for c in children(n) {
            self.walk(c);
        }
    }

    fn literal_or_comment(&mut self, n: Node, t: &str, is_string: bool) {
        if let Some(c) = t.chars().find(|&c| is_decorative(c)) {
            let place = if is_string { "string" } else { "comment" };
            self.hit("decorative-unicode", n, format!("`{c}` in a {place}"));
        }
        if is_string && is_color(t) {
            self.hit("hardcoded-color", n, format!("color {t} hard-coded; take it from a theme or palette"));
        }
    }

    fn call(&mut self, n: Node, k: &str) {
        let callee = n.child_by_field_name("function").or_else(|| n.child_by_field_name("macro")).or_else(|| n.named_child(0));
        let name = callee.map(|c| text(c, self.src)).unwrap_or("");
        let name = if k == "macro_invocation" { format!("{name}!") } else { name.to_string() };
        if DEBUG_PRINTS.contains(&name.as_str()) {
            self.hit("debug-print", n, format!("`{name}` left in code; use the project logger"));
        }
        let last = name.rsplit(['.', ':']).next().unwrap_or("");
        if SLEEPS.contains(&last) && NUMBER_KINDS.iter().any(|nk| contains_kind(n, nk)) {
            self.hit("hardcoded-delay", n, format!("`{last}` with a literal delay; wait on the event instead"));
        }
    }

    fn error_only_printed(&mut self, n: Node<'t>, k: &str) {
        let body = if self.spec.catches.contains(&k) {
            self.catch_body(n)
        } else if self.lang == Lang::Go && k == "if_statement" {
            n.child_by_field_name("condition").filter(|c| text(*c, self.src).ends_with("err != nil")).and(n.child_by_field_name("consequence"))
        } else if k == "match_arm" {
            n.child_by_field_name("pattern").filter(|p| text(*p, self.src).starts_with("Err(")).and(n.child_by_field_name("value"))
        } else {
            None
        };
        let Some(body) = body else { return };
        if self.at_boundary(n, body) {
            return;
        }
        let stmts = self.statements_or_self(body);
        let prints = |x: &Node| ERROR_PRINTS.iter().any(|p| text(*x, self.src).starts_with(p));
        let drops = |x: &Node| x.kind() == "pass_statement" || (self.is_return(*x) && !text(*x, self.src).contains("err"));
        if stmts.iter().any(prints) && stmts.iter().all(|x| prints(x) || drops(x)) {
            self.hit("error-only-printed", n, "error is printed and dropped; handle it or propagate it".into());
        }
    }

    /// Whether the error has nowhere further to go, so logging it is the
    /// handling: a Go function without an `error` result that returns on the
    /// spot or whose failed call's results die with the `if`, or a TS/JS
    /// callback or `void` function. Printing and then going on to use a failed
    /// result, returning from `main` (exit status 0), or returning from an HTTP
    /// handler without a response are still flagged.
    fn at_boundary(&self, n: Node<'t>, body: Node<'t>) -> bool {
        let mut f = n.parent();
        while let Some(x) = f.filter(|x| !self.spec.functions.contains(&x.kind()) && !self.spec.lambdas.contains(&x.kind())) {
            f = x.parent();
        }
        let Some(f) = f else { return false };
        let field = |name: &str| f.child_by_field_name(name).map_or("", |x| text(x, self.src));
        if self.lang == Lang::Go {
            let returns = self.statements_or_self(body).last().is_some_and(|l| self.is_return(*l));
            let scoped = n.child_by_field_name("initializer").is_some();
            let error_result = field("result").split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == "error");
            let handler = field("parameters").contains("http.ResponseWriter");
            let main = f.kind() == "function_declaration" && field("name") == "main";
            return !error_result && (returns || scoped) && !(returns && (main || handler));
        }
        if self.lang.is_js_like() {
            let ret = field("return_type").trim_start_matches(':').trim();
            let callback = ret.is_empty() && f.parent().is_some_and(|p| p.kind() == "arguments");
            return callback || matches!(ret, "void" | "Promise<void>");
        }
        false
    }

    fn in_const(&self, n: Node) -> bool {
        let mut cur = n.parent();
        for _ in 0..4 {
            let Some(p) = cur else { return false };
            if CONST_CONTEXTS.contains(&p.kind()) {
                return true;
            }
            if matches!(p.kind(), "assignment" | "variable_declarator" | "property_declaration") {
                let name = p.child_by_field_name("left").or_else(|| p.child_by_field_name("name")).map_or("", |x| text(x, self.src));
                return name.len() > 1 && name.chars().all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit());
            }
            cur = p.parent();
        }
        false
    }

    fn statements(&self, block: Node<'t>) -> Vec<Node<'t>> {
        named_children(block)
            .into_iter()
            .flat_map(|c| match c.kind() {
                "statement_list" | "statements" | "block" => self.statements(c),
                _ => vec![c],
            })
            .filter(|c| !self.spec.comments.contains(&c.kind()) && !is_docstring(*c))
            .collect()
    }

    fn statements_or_self(&self, n: Node<'t>) -> Vec<Node<'t>> {
        if self.spec.blocks.contains(&n.kind()) || n.kind() == "statements" { self.statements(n) } else { vec![n] }
    }

    fn if_parts(&self, n: Node<'t>) -> (Option<Node<'t>>, Option<Node<'t>>, bool) {
        if self.lang == Lang::Swift {
            let (mut cons, mut alt, mut after_else) = (None, None, false);
            for c in children(n) {
                if c.kind() == "else" {
                    after_else = true;
                } else if matches!(c.kind(), "statements" | "if_statement") {
                    if after_else {
                        alt = Some(c);
                    } else if cons.is_none() {
                        cons = Some(c);
                    }
                }
            }
            return (cons, alt, false);
        }
        let mut cur = n.walk();
        let alts: Vec<Node> = n.children_by_field_name("alternative", &mut cur).collect();
        let alt = alts.last().map(|a| match a.kind() {
            "else_clause" => a.child_by_field_name("body").or_else(|| a.named_child(0)).unwrap_or(*a),
            _ => *a,
        });
        let cons = n.child_by_field_name("consequence").or_else(|| n.child_by_field_name("body"));
        (cons, alt, alts.iter().any(|a| self.spec.elifs.contains(&a.kind())))
    }

    fn is_if(&self, n: Node) -> bool {
        matches!(n.kind(), "if_statement" | "if_expression")
    }

    fn is_return(&self, n: Node) -> bool {
        matches!(n.kind(), "return_statement" | "return_expression")
            || (n.kind() == "control_transfer_statement" && text(n, self.src).starts_with("return"))
            || (n.kind() == "expression_statement" && n.named_child(0).is_some_and(|c| c.kind() == "return_expression"))
    }

    fn staircase(&self, block: Node<'t>) -> u32 {
        let stmts = self.statements_or_self(block);
        if !stmts.last().is_some_and(|l| self.is_return(*l)) {
            return 0;
        }
        let inner = stmts.len().checked_sub(2).map(|i| stmts[i]).filter(|p| self.is_if(*p) && self.if_parts(*p).1.is_none());
        1 + inner.and_then(|p| self.if_parts(p).0).map_or(0, |b| self.staircase(b))
    }

    fn duplicate_conditions(&mut self, head: Node<'t>) {
        let mut seen: Vec<String> = vec![];
        let mut cur = Some(head);
        while let Some(n) = cur {
            let mut conds = vec![n.child_by_field_name("condition")];
            conds.extend(children(n).into_iter().filter(|c| self.spec.elifs.contains(&c.kind())).map(|c| c.child_by_field_name("condition")));
            for c in conds.into_iter().flatten() {
                let t = squash(text(c, self.src));
                if seen.contains(&t) {
                    self.hit("duplicate-condition", c, "condition repeats an earlier branch, so this branch never runs".into());
                } else {
                    seen.push(t);
                }
            }
            cur = self.if_parts(n).1.filter(|a| self.is_if(*a));
        }
    }

    fn ternary(&mut self, n: Node) {
        let named = named_children(n);
        if named.iter().any(|c| self.spec.ternaries.contains(&c.kind())) {
            self.hit("nested-ternary", n, "nested ternary; use if/else or a lookup".into());
        }
        let branches = match (
            n.child_by_field_name("consequence").or(n.child_by_field_name("if_true")),
            n.child_by_field_name("alternative").or(n.child_by_field_name("if_false")),
        ) {
            (Some(a), Some(b)) => Some((a, b)),
            _ if named.len() == 3 => Some((named[0], named[2])),
            _ => None,
        };
        if branches.is_some_and(|(a, b)| squash(text(a, self.src)) == squash(text(b, self.src))) {
            self.hit("identical-branches", n, "both ternary branches are identical".into());
        }
    }

    fn catch_body(&self, n: Node<'t>) -> Option<Node<'t>> {
        n.child_by_field_name("body").or_else(|| named_children(n).into_iter().find(|c| matches!(c.kind(), "block" | "statements" | "statement_block")))
    }

    fn catch(&mut self, n: Node<'t>) {
        let empty = self.catch_body(n).is_none_or(|b| {
            let kids = named_children(b);
            kids.iter().all(|c| c.kind() == "pass_statement")
        });
        if empty {
            self.hit("empty-catch", n, "exception swallowed without handling or a comment".into());
        }
        if self.lang == Lang::Python {
            let head = text(n, self.src).split(':').next().unwrap_or("").trim();
            if head == "except" || head.starts_with("except Exception") || head.starts_with("except BaseException") {
                self.hit("broad-except", n, "catches every exception".into());
            }
        }
    }
}

fn is_color(t: &str) -> bool {
    let inner = t.trim_matches(|c| c == '"' || c == '\'' || c == '`');
    let hex = inner.starts_with('#') && matches!(inner.len(), 4 | 5 | 7 | 9) && inner[1..].chars().all(|c| c.is_ascii_hexdigit());
    let rgb = (inner.starts_with("rgb(") || inner.starts_with("rgba(")) && inner.ends_with(')') && inner.contains(|c: char| c.is_ascii_digit());
    hex || rgb
}

fn contains_kind(n: Node, kind: &str) -> bool {
    n.kind() == kind || children(n).into_iter().any(|c| contains_kind(c, kind))
}

fn number_context(n: Node) -> &'static str {
    let mut p = n.parent();
    while let Some(x) = p.filter(|x| {
        matches!(x.kind(), "unary_expression" | "unary_operator" | "prefix_expression" | "parenthesized_expression" | "value_argument" | "argument")
    }) {
        p = x.parent();
    }
    match p.map_or("", |x| x.kind()) {
        "argument_list" | "arguments" | "value_arguments" | "keyword_argument" | "call_suffix" => "call argument",
        "binary_expression" | "binary_operator" | "additive_expression" | "multiplicative_expression" | "range_expression" => "arithmetic",
        "array" | "list" | "tuple" | "set" | "array_expression" | "tuple_expression" | "array_literal" | "literal_value"
        | "literal_element" | "expression_list" | "dictionary_literal" | "dictionary" | "pair" | "keyed_element"
        | "array_creation_expression" | "array_element_initializer" => "collection",
        "assignment" | "assignment_expression" | "augmented_assignment" | "variable_declarator" | "let_declaration"
        | "short_var_declaration" | "var_spec" | "property_declaration" | "compound_assignment_expr"
        | "augmented_assignment_expression" | "field_initializer" | "public_field_definition" => "assignment",
        _ => "other",
    }
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_decorative(c: char) -> bool {
    matches!(
        c as u32,
        0x1F300..=0x1FAFF | 0x2705 | 0x274C | 0x26A0 | 0x2728 | 0x2B50 | 0x26A1 | 0x23F3 | 0x2753 | 0x2757 | 0x2190..=0x21FF | 0x2500..=0x25FF | 0x2022 | 0x2713..=0x2718
    )
}
