// sloplint: ignore[hardcoded-color] the palette hosts are told apart by
const presence = {
  blue: "#1a73e8",
  red: "#d93025",
  green: "#188038",
};

function watch(source: EventSource): void {
  source.addEventListener("presence", (event) => {
    try {
      apply(JSON.parse(event.data));
    } catch (error) {
      console.error(error);
    }
  });
}

async function save(body: unknown): Promise<void> {
  try {
    await post(body);
  } catch (error) {
    console.error(error);
  }
}
