import assert from "node:assert/strict";
import test from "node:test";
import { readdir, readFile } from "node:fs/promises";

const frontendDirectory = new URL("./", import.meta.url);
const frontendSource = (
  await Promise.all(
    (await readdir(frontendDirectory))
      .filter((name) => name.endsWith(".ts"))
      .map((name) => readFile(new URL(name, frontendDirectory), "utf8")),
  )
).join("\n");
const tauriSource = await readFile(new URL("../src-tauri/src/main.rs", import.meta.url), "utf8");

test("GUI content stays in plain text mode", () => {
  assert.match(frontendSource, /setMarkdownText\(worldPremise,/u);
  assert.doesNotMatch(frontendSource, /\binnerHTML\b/u);
  assert.doesNotMatch(frontendSource, /\bdangerouslySetInnerHTML\b/u);
  assert.doesNotMatch(frontendSource, /\binsertAdjacentHTML\b/u);
});

test("desktop sources avoid default console or stdout logging", () => {
  assert.doesNotMatch(frontendSource, /\bconsole\.(?:log|warn|error|info|debug)\b/u);
  assert.doesNotMatch(tauriSource, /\b(?:println!|eprintln!|dbg!)\b/u);
});
