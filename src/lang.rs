use serde::Serialize;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Python,
    Rust,
    Go,
    JavaScript,
    TypeScript,
    Tsx,
    Swift,
    Php,
}

pub struct Spec {
    pub functions: &'static [&'static str],
    pub lambdas: &'static [&'static str],
    pub ifs: &'static [&'static str],
    pub elifs: &'static [&'static str],
    pub elses: &'static [&'static str],
    pub loops: &'static [&'static str],
    pub switches: &'static [&'static str],
    pub cases: &'static [&'static str],
    pub catches: &'static [&'static str],
    pub ternaries: &'static [&'static str],
    pub bool_nodes: &'static [&'static str],
    pub bool_ops: &'static [&'static str],
    pub comments: &'static [&'static str],
    pub strings: &'static [&'static str],
    pub literals: &'static [&'static str],
    pub identifiers: &'static [&'static str],
    pub blocks: &'static [&'static str],
    pub imports: &'static [&'static str],
    pub clone_roots: &'static [&'static str],
}

const PYTHON: Spec = Spec {
    functions: &["function_definition"],
    lambdas: &["lambda"],
    ifs: &["if_statement"],
    elifs: &["elif_clause"],
    elses: &["else_clause"],
    loops: &["for_statement", "while_statement"],
    switches: &["match_statement"],
    cases: &["case_clause"],
    catches: &["except_clause", "except_group_clause"],
    ternaries: &["conditional_expression", "if_clause"],
    bool_nodes: &["boolean_operator"],
    bool_ops: &["and", "or"],
    comments: &["comment"],
    strings: &["string"],
    literals: &["string", "integer", "float", "true", "false", "none"],
    identifiers: &["identifier"],
    blocks: &["block", "module"],
    imports: &["import_statement", "import_from_statement", "future_import_statement"],
    clone_roots: &[
        "function_definition", "if_statement", "for_statement", "while_statement",
        "with_statement", "try_statement", "match_statement",
    ],
};

const RUST: Spec = Spec {
    functions: &["function_item"],
    lambdas: &["closure_expression"],
    ifs: &["if_expression"],
    elifs: &[],
    elses: &["else_clause"],
    loops: &["for_expression", "while_expression", "loop_expression"],
    switches: &["match_expression"],
    cases: &["match_arm"],
    catches: &[],
    ternaries: &[],
    bool_nodes: &["binary_expression"],
    bool_ops: &["&&", "||"],
    comments: &["line_comment", "block_comment"],
    strings: &["string_literal", "raw_string_literal"],
    literals: &[
        "string_literal", "raw_string_literal", "char_literal", "integer_literal",
        "float_literal", "boolean_literal",
    ],
    identifiers: &["identifier", "field_identifier", "type_identifier"],
    blocks: &["block", "source_file", "declaration_list"],
    imports: &["use_declaration", "extern_crate_declaration", "mod_item"],
    clone_roots: &[
        "function_item", "if_expression", "for_expression", "while_expression",
        "loop_expression", "match_expression", "closure_expression",
    ],
};

const GO: Spec = Spec {
    functions: &["function_declaration", "method_declaration"],
    lambdas: &["func_literal"],
    ifs: &["if_statement"],
    elifs: &[],
    elses: &[],
    loops: &["for_statement"],
    switches: &["expression_switch_statement", "type_switch_statement", "select_statement"],
    cases: &["expression_case", "type_case", "communication_case"],
    catches: &[],
    ternaries: &[],
    bool_nodes: &["binary_expression"],
    bool_ops: &["&&", "||"],
    comments: &["comment"],
    strings: &["interpreted_string_literal", "raw_string_literal"],
    literals: &[
        "interpreted_string_literal", "raw_string_literal", "rune_literal", "int_literal",
        "float_literal", "imaginary_literal", "true", "false", "nil", "iota",
    ],
    identifiers: &["identifier", "field_identifier", "type_identifier", "package_identifier"],
    blocks: &["block", "statement_list", "source_file"],
    imports: &["import_declaration", "package_clause"],
    clone_roots: &[
        "function_declaration", "method_declaration", "func_literal", "if_statement",
        "for_statement", "expression_switch_statement", "type_switch_statement",
        "select_statement",
    ],
};

const JS_FUNCTIONS: &[&str] = &[
    "function_declaration", "generator_function_declaration", "method_definition",
];
const JS_LAMBDAS: &[&str] = &["arrow_function", "function_expression", "function", "generator_function"];
const JS_CLONE_ROOTS: &[&str] = &[
    "function_declaration", "generator_function_declaration", "method_definition",
    "arrow_function", "function_expression", "function", "if_statement", "for_statement",
    "for_in_statement", "while_statement", "do_statement", "switch_statement", "try_statement",
];

const fn js_like(literals: &'static [&'static str], identifiers: &'static [&'static str]) -> Spec {
    Spec {
        functions: JS_FUNCTIONS,
        lambdas: JS_LAMBDAS,
        ifs: &["if_statement"],
        elifs: &[],
        elses: &["else_clause"],
        loops: &["for_statement", "for_in_statement", "while_statement", "do_statement"],
        switches: &["switch_statement"],
        cases: &["switch_case"],
        catches: &["catch_clause"],
        ternaries: &["ternary_expression"],
        bool_nodes: &["binary_expression"],
        bool_ops: &["&&", "||", "??"],
        comments: &["comment"],
        strings: &["string", "template_string"],
        literals,
        identifiers,
        blocks: &["statement_block", "program", "class_body"],
                imports: &["import_statement"],
        clone_roots: JS_CLONE_ROOTS,
    }
}

const JS_LITERALS: &[&str] = &[
    "string", "template_string", "number", "regex", "true", "false", "null", "undefined",
];
const JAVASCRIPT: Spec = js_like(
    JS_LITERALS,
    &["identifier", "property_identifier", "shorthand_property_identifier"],
);
const TYPESCRIPT: Spec = js_like(
    JS_LITERALS,
    &["identifier", "property_identifier", "shorthand_property_identifier", "type_identifier"],
);

const SWIFT: Spec = Spec {
    functions: &[
        "function_declaration", "init_declaration", "deinit_declaration",
        "subscript_declaration", "computed_property",
    ],
    lambdas: &["lambda_literal"],
    ifs: &["if_statement", "guard_statement"],
    elifs: &[],
    elses: &[],
    loops: &["for_statement", "while_statement", "repeat_while_statement"],
    switches: &["switch_statement"],
    cases: &["switch_entry"],
    catches: &["catch_block"],
    ternaries: &["ternary_expression"],
    bool_nodes: &["conjunction_expression", "disjunction_expression"],
    bool_ops: &["&&", "||"],
    comments: &["comment", "multiline_comment"],
    strings: &["line_string_literal", "multi_line_string_literal", "raw_string_literal"],
    literals: &[
        "line_string_literal", "multi_line_string_literal", "raw_string_literal",
        "integer_literal", "real_literal", "hex_literal", "bin_literal", "oct_literal",
        "boolean_literal", "nil",
    ],
    identifiers: &["simple_identifier", "type_identifier"],
    blocks: &["statements", "source_file", "class_body"],
    imports: &["import_declaration"],
    clone_roots: &[
        "function_declaration", "init_declaration", "computed_property", "if_statement",
        "guard_statement", "for_statement", "while_statement", "switch_statement",
        "do_statement", "lambda_literal",
    ],
};

const PHP: Spec = Spec {
    functions: &["function_definition", "method_declaration"],
    lambdas: &["anonymous_function", "arrow_function"],
    ifs: &["if_statement"],
    elifs: &["else_if_clause"],
    elses: &["else_clause"],
    loops: &["for_statement", "foreach_statement", "while_statement", "do_statement"],
    switches: &["switch_statement", "match_expression"],
    cases: &["case_statement", "match_conditional_expression"],
    catches: &["catch_clause"],
    ternaries: &["conditional_expression"],
    bool_nodes: &["binary_expression"],
    bool_ops: &["&&", "||", "and", "or"],
    comments: &["comment"],
    strings: &["string", "encapsed_string", "heredoc", "nowdoc"],
    literals: &["string", "encapsed_string", "heredoc", "nowdoc", "integer", "float", "boolean", "null"],
    identifiers: &["name"],
    blocks: &["compound_statement", "program", "declaration_list"],
    imports: &["namespace_use_declaration"],
    clone_roots: &[
        "function_definition", "method_declaration", "anonymous_function", "if_statement", "for_statement",
        "foreach_statement", "while_statement", "do_statement", "switch_statement", "try_statement",
    ],
};

impl Lang {
    pub fn from_path(path: &Path) -> Option<Lang> {
        Some(match path.extension()?.to_str()? {
            "py" | "pyw" => Lang::Python,
            "rs" => Lang::Rust,
            "go" => Lang::Go,
            "js" | "mjs" | "cjs" | "jsx" => Lang::JavaScript,
            "ts" | "mts" | "cts" => Lang::TypeScript,
            "tsx" => Lang::Tsx,
            "swift" => Lang::Swift,
            "php" => Lang::Php,
            _ => return None,
        })
    }

    pub fn grammar(self) -> tree_sitter::Language {
        match self {
            Lang::Python => tree_sitter_python::LANGUAGE.into(),
            Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
            Lang::Go => tree_sitter_go::LANGUAGE.into(),
            Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Lang::Swift => tree_sitter_swift::LANGUAGE.into(),
            Lang::Php => tree_sitter_php::LANGUAGE_PHP.into(),
        }
    }

    pub fn spec(self) -> &'static Spec {
        match self {
            Lang::Python => &PYTHON,
            Lang::Rust => &RUST,
            Lang::Go => &GO,
            Lang::JavaScript => &JAVASCRIPT,
            Lang::TypeScript | Lang::Tsx => &TYPESCRIPT,
            Lang::Swift => &SWIFT,
            Lang::Php => &PHP,
        }
    }

    pub fn is_js_like(self) -> bool {
        matches!(self, Lang::JavaScript | Lang::TypeScript | Lang::Tsx)
    }
}

pub fn is_test_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let stem = name.split('.').next().unwrap_or("");
    s.split('/').any(|c| matches!(c, "test" | "tests" | "__tests__" | "Tests" | "testdata" | "spec" | "e2e"))
        || stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with("Tests")
        || stem.ends_with("Test")
        || name.contains(".test.")
        || name.contains(".spec.")
        || name == "conftest.py"
}
