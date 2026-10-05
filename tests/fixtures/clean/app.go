package app

const marginRatio = 0.05

func load(path string) ([]byte, error) {
	data, err := read(path)
	if err != nil {
		return nil, fmt.Errorf("load %s: %w", path, err)
	}
	return data, nil
}
