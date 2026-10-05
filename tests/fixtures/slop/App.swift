func layout(width: Double) -> Double {
    let margin = width * 0.05
    let header = width * 0.12 + 14
    let cols = [3, 7, 11]
    return draw(margin, header, cols, 640, 480)
}

let primary = "#1e90ff"

func load(path: String) {
    do {
        try read(path)
    } catch {
        print("Error: \(error)")
    }
}
