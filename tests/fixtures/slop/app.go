package app

func layout(width float64) float64 {
	margin := width * 0.05
	header := width*0.12 + 14
	cols := []int{3, 7, 11}
	return draw(margin, header, cols, 640, 480)
}

func theme() string { return "#1e90ff" }

func load(path string) {
	data, err := read(path)
	if err != nil {
		fmt.Println(err)
	}
	use(data)
}
