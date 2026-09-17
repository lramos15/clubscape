import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { lstatSync } from "node:fs";

const root = resolve(fileURLToPath(new URL("../../", import.meta.url)));

/** Keep candidate builds separate from preserved running/reproduction roots. */
export function webOutputDirectory(value = process.env.CLUBSCAPE_WEB_OUTPUT): string {
  const relative = value ?? "web/dist";
  if (value !== undefined && !/^\.local\/web-builds\/[a-z0-9][a-z0-9_-]{0,95}$/.test(value)) {
    throw new Error("CLUBSCAPE_WEB_OUTPUT must name a bounded .local/web-builds/<candidate> directory.");
  }
  let directory = root;
  for (const part of relative.split("/")) {
    directory = resolve(directory, part);
    let stat;
    try { stat = lstatSync(directory); }
    catch (error) {
      if (error instanceof Error && "code" in error && error.code === "ENOENT") continue;
      throw error;
    }
    if (stat.isSymbolicLink() || !stat.isDirectory()) throw new Error("A web build output cannot traverse a symlink or non-directory.");
  }
  return directory;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  console.log(webOutputDirectory());
}
