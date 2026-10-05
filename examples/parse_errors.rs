fn error_bytes(n: tree_sitter::Node) -> usize {
    if n.is_error() || n.is_missing() {
        return n.byte_range().len();
    }
    let mut c = n.walk();
    n.children(&mut c).filter(|c| c.has_error() || c.is_missing()).map(error_bytes).sum()
}

fn main() {
    for path in std::io::stdin().lines().map_while(Result::ok) {
        let ext = path.rsplit('.').next().unwrap_or("");
        let lang: tree_sitter::Language = match ext {
            "swift" => tree_sitter_swift::LANGUAGE.into(),
            "ts" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
            "js" | "mjs" | "jsx" => tree_sitter_javascript::LANGUAGE.into(),
            "py" => tree_sitter_python::LANGUAGE.into(),
            "rs" => tree_sitter_rust::LANGUAGE.into(),
            "go" => tree_sitter_go::LANGUAGE.into(),
            "php" => tree_sitter_php::LANGUAGE_PHP.into(),
            _ => continue,
        };
        let src = std::fs::read_to_string(&path).unwrap_or_default();
        let mut p = tree_sitter::Parser::new();
        p.set_language(&lang).unwrap();
        let t = p.parse(&src, None).unwrap();
        let e = error_bytes(t.root_node());
        if e > 0 {
            println!("{:.3}\t{}", e as f64 / src.len().max(1) as f64, path);
        }
    }
}
