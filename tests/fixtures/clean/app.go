package app

const marginRatio = 0.05

func load(path string) ([]byte, error) {
	data, err := read(path)
	if err != nil {
		return nil, fmt.Errorf("load %s: %w", path, err)
	}
	return data, nil
}

// sloplint: ignore[hardcoded-color] the palette hosts are told apart by
var presence = []string{
	"#1a73e8",
	"#d93025",
	"#188038",
}

func warm() {
	go func() {
		if err := compile(); err != nil {
			log.Printf("compile: %v", err)
		}
	}()
}

func notify(id int64) {
	name, err := lookup(id)
	if err != nil {
		log.Printf("notify: lookup %d: %v", id, err)
		return
	}
	send(name)
}
