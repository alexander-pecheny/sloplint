fn layout(width: f64) -> f64 {
    let margin = width * 0.05;
    let header = width * 0.12 + 14.0;
    let cols = [3, 7, 11];
    draw(margin, header, &cols, 640, 480)
}

fn theme() -> &'static str { "#1e90ff" }

fn load(path: &str) {
    match std::fs::read_to_string(path) {
        Ok(s) => use_it(s),
        Err(e) => eprintln!("error: {e}"),
    }
}
