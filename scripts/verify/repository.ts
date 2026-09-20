import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { posix } from "node:path";
import { privatePath, publicDocumentPath } from "./policy";

const args = process.argv.slice(2);
if (args.some((arg) => arg !== "--staged")) {
  throw new Error("Usage: bun scripts/verify/repository.ts [--staged]");
}
const staged = args.includes("--staged");
const git = (...args: string[]) =>
  execFileSync("git", args, { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
const files = new Set(
  git(
    "ls-files",
    "-z",
    "--cached",
    ...(staged ? [] : ["--others", "--exclude-standard"]),
  )
    .split("\0")
    .filter((file) => file && (staged || existsSync(file))),
);
const errors: string[] = [];
for (const file of files) {
  if (privatePath(file)) errors.push(`Private or generated path: ${file}`);
  else if (!publicDocumentPath(file))
    errors.push(
      `Move the public document into the documentation structure: ${file}`,
    );
}
const read = (file: string) =>
  staged ? git("show", `:${file}`) : readFileSync(file, "utf8");
const documents = new Map(
  [...files]
    .filter((file) => file.endsWith(".md") && !privatePath(file))
    .map((file) => [file, read(file)]),
);

function withoutCode(text: string) {
  return text.replace(/^(`{3,}|~{3,})[^\n]*\n[\s\S]*?^\1\s*$/gm, "");
}

function anchors(text: string): Set<string> {
  const ids = new Set<string>();
  for (const match of withoutCode(text).matchAll(/^#{1,6}\s+(.+?)\s*#*$/gm)) {
    const base = match[1]
      .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
      .toLowerCase()
      .replace(/[^\p{L}\p{N}_\-\s]/gu, "")
      .replace(/\s/g, "-");
    let id = base;
    for (let count = 1; ids.has(id); count++) id = `${base}-${count}`;
    ids.add(id);
  }
  for (const match of text.matchAll(
    /<(?:a|[a-z][\w-]*)\b[^>]*\b(?:id|name)=["']([^"']+)["']/gi,
  ))
    ids.add(match[1]);
  return ids;
}

for (const [file, text] of documents) {
  const content = withoutCode(text);
  const links = [
    ...content.matchAll(
      /\[[^\]\n]*\]\(<?([^\s)>]+)>?(?:\s+["'][^\n]*["'])?\)/g,
    ),
    ...content.matchAll(/^\s*\[[^\]]+\]:\s*<?([^\s>]+)>?/gm),
  ];
  for (const match of links) {
    const link = match[1];
    if (/^(?:[a-z][a-z\d+.-]*:|\/\/)/i.test(link)) continue;
    const [target, fragment] = link.split("#", 2);
    let destination: string;
    try {
      destination = target
        ? posix.normalize(
            posix.join(posix.dirname(file), decodeURIComponent(target)),
          )
        : file;
    } catch {
      errors.push(`${file}: invalid link ${link}`);
      continue;
    }
    if (privatePath(destination)) {
      errors.push(`${file}: link to private path ${link}`);
      continue;
    }
    if (!files.has(destination)) {
      const index = posix.join(destination, "README.md");
      if (files.has(index)) destination = index;
      else {
        errors.push(`${file}: missing public link ${link}`);
        continue;
      }
    }
    if (fragment && documents.has(destination)) {
      let id: string;
      try {
        id = decodeURIComponent(fragment);
      } catch {
        errors.push(`${file}: invalid anchor ${link}`);
        continue;
      }
      if (!anchors(documents.get(destination)!).has(id))
        errors.push(`${file}: missing anchor ${link}`);
    }
  }
}
if (errors.length) {
  console.error(errors.join("\n"));
  process.exitCode = 1;
} else {
  console.log(
    `Repository check passed: ${files.size} files, ${documents.size} documents${staged ? " in the index" : ""}.`,
  );
}
