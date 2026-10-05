function layout(width: number): number {
  const margin = width * 0.05;
  const header = width * 0.12 + 14;
  const cols = [3, 7, 11];
  return draw(margin, header, cols, 640, 480);
}

const theme = { primary: "#1e90ff" };

async function load(path: string) {
  try {
    return await read(path);
  } catch (e) {
    console.error("Error:", e);
  }
}
