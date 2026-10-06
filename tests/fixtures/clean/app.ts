const presence = ["#1a73e8", "#d93025", "#188038"];

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
