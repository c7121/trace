#!/usr/bin/env node
/**
 * Sync shared agent standards from a checked-out canonical repo into the current repo.
 *
 * Usage:
 *   node scripts/sync_agent_standards.mjs <path-to-agent-standards-repo>
 *
 * Goals:
 * - Deterministic and minimal: copy only a fixed file set.
 * - No network: the workflow checks out upstream; this script only copies.
 *
 * Why .mjs:
 * - Runs as ESM with plain Node.js (no TS loaders/transpilation).
 */

import * as fs from "node:fs";
import * as path from "node:path";

const FILES = [
  "docs/agent/AGENTS.shared.md",
  "docs/agent/checklist.md",
  "docs/agent/references.md",
  "docs/specs/_mini_template.md",
  "docs/specs/_template.md",
  "docs/adr/_template.md",
];

function die(message) {
  console.error(message);
  process.exit(2);
}

function exists(filePath) {
  try {
    fs.accessSync(filePath);
    return true;
  } catch {
    return false;
  }
}

function readBytes(filePath) {
  return fs.readFileSync(filePath);
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function main() {
  const args = process.argv.slice(2);
  if (args.length !== 1) {
    die("Usage: node scripts/sync_agent_standards.mjs <path-to-agent-standards-repo>");
  }

  const srcRoot = path.resolve(args[0]);
  const dstRoot = process.cwd();

  if (!exists(srcRoot)) die(`Upstream path does not exist: ${srcRoot}`);

  let changed = false;

  for (const rel of FILES) {
    const src = path.join(srcRoot, rel);
    const dst = path.join(dstRoot, rel);

    if (!exists(src)) {
      console.warn(`Missing in upstream (skipping): ${rel}`);
      continue;
    }

    const srcBytes = readBytes(src);
    if (exists(dst)) {
      const dstBytes = readBytes(dst);
      if (Buffer.compare(srcBytes, dstBytes) === 0) continue;
    }

    ensureDir(path.dirname(dst));
    fs.writeFileSync(dst, srcBytes);
    changed = true;
    console.log(`Updated: ${rel}`);
  }

  if (!changed) console.log("No changes.");
}

main();
