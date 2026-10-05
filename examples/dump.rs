use std::io::Read;
fn main() {
    let lang = std::env::args().nth(1).unwrap();
    let mut src = String::new();
    std::io::stdin().read_to_string(&mut src).unwrap();
    let l: tree_sitter::Language = match lang.as_str() {
        "py" => tree_sitter_python::LANGUAGE.into(),
        "rs" => tree_sitter_rust::LANGUAGE.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        "js" => tree_sitter_javascript::LANGUAGE.into(),
        "ts" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "swift" => tree_sitter_swift::LANGUAGE.into(),
        "php" => tree_sitter_php::LANGUAGE_PHP.into(),
        _ => panic!(),
    };
    let mut p = tree_sitter::Parser::new();
    p.set_language(&l).unwrap();
    let t = p.parse(&src, None).unwrap();
    fn walk(n: tree_sitter::Node, d: usize, src: &str, field: Option<&str>) {
        let txt = if n.child_count() == 0 { format!(" {:?}", &src[n.byte_range()]) } else { String::new() };
        println!("{}{}{}{}", "  ".repeat(d), field.map(|f| format!("{f}: ")).unwrap_or_default(), n.kind(), txt);
        let mut c = n.walk();
        for (i, ch) in n.children(&mut c).enumerate() {
            walk(ch, d + 1, src, n.field_name_for_child(i as u32));
        }
    }
    walk(t.root_node(), 0, &src, None);
}
