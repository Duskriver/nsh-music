#!/usr/bin/env node
// NSH i18n typegen and static linter (`generate` / `check`).
// Zero external dependencies; matches rust-i18n's `_version: 1` and `_version: 2`
// layouts under `locales/`.

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const ROOT = process.cwd();
const LOCALES_DIR = path.join(ROOT, "locales");
const GENERATED_PATH = path.join(ROOT, "src/i18n/translations.generated.ts");
const BASE_LOCALE = "en";
const ALLOWED_NATIVE_IMPORT_FILE = path.join(ROOT, "src/i18n/index.ts");

function fail(message) {
  console.error(`i18n error: ${message}`);
  process.exitCode = 1;
}

function listFiles(dir, exts) {
  if (!fs.existsSync(dir)) return [];
  const out = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) out.push(...listFiles(full, exts));
    else if (exts.some((ext) => entry.name.endsWith(ext))) out.push(full);
  }
  return out.sort();
}

function stripComment(line) {
  let quote = null;
  let escaped = false;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (quote === '"' && ch === "\\") {
      escaped = true;
      continue;
    }
    if (quote) {
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === "'" || ch === '"') {
      quote = ch;
      continue;
    }
    if (ch === "#" && (i === 0 || /\s/.test(line[i - 1]))) {
      return line.slice(0, i);
    }
  }
  return line;
}

function findUnquotedColon(line) {
  let quote = null;
  let escaped = false;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (quote === '"' && ch === "\\") {
      escaped = true;
      continue;
    }
    if (quote) {
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === "'" || ch === '"') {
      quote = ch;
      continue;
    }
    if (ch === ":" && (i + 1 === line.length || /\s/.test(line[i + 1]))) {
      return i;
    }
  }
  return -1;
}

function unquoteKey(raw) {
  const trimmed = raw.trim();
  if (trimmed.startsWith('"') && trimmed.endsWith('"')) return JSON.parse(trimmed);
  if (trimmed.startsWith("'") && trimmed.endsWith("'")) return trimmed.slice(1, -1).replace(/''/g, "'");
  return trimmed;
}

function parseScalar(raw) {
  const trimmed = raw.trim();
  if (trimmed === "" || trimmed === "~" || trimmed === "null") return "";
  if (trimmed.startsWith('"') && trimmed.endsWith('"')) return JSON.parse(trimmed);
  if (trimmed.startsWith("'") && trimmed.endsWith("'")) return trimmed.slice(1, -1).replace(/''/g, "'");
  if (/^-?\d+$/.test(trimmed)) return Number(trimmed);
  if (trimmed === "true") return true;
  if (trimmed === "false") return false;
  return trimmed;
}

function parseYamlFlat(filePath) {
  const content = fs.readFileSync(filePath, "utf8");
  const lines = content.split(/\r?\n/);
  const stack = [{ indent: -1, prefix: [] }];
  const entries = new Map();
  let version = 1;

  for (let lineIdx = 0; lineIdx < lines.length; lineIdx++) {
    const rawLine = lines[lineIdx];
    const noComment = stripComment(rawLine);
    if (noComment.trim() === "") continue;
    if (noComment.trim() === "---") continue;

    const indent = noComment.match(/^ */)[0].length;
    if (rawLine.includes("\t")) {
      throw new Error(`${filePath}:${lineIdx + 1}: tabs are not supported in YAML indentation`);
    }
    const contentPart = noComment.slice(indent);
    const colon = findUnquotedColon(contentPart);
    if (colon === -1) {
      throw new Error(`${filePath}:${lineIdx + 1}: expected "key: value"`);
    }

    const key = unquoteKey(contentPart.slice(0, colon));
    const rest = contentPart.slice(colon + 1).trim();

    while (stack.length > 1 && indent <= stack[stack.length - 1].indent) {
      stack.pop();
    }

    const fullParts = [...stack[stack.length - 1].prefix, key];
    if (rest === "") {
      stack.push({ indent, prefix: fullParts });
      continue;
    }

    const joined = fullParts.join(".");
    const val = parseScalar(rest);
    if (joined === "_version") {
      version = Number(val);
      continue;
    }
    if (entries.has(joined)) {
      throw new Error(`${filePath}:${lineIdx + 1}: duplicate key "${joined}"`);
    }
    entries.set(joined, String(val));
  }

  return { version, entries };
}

function flattenJson(obj, prefix = [], out = new Map(), filePath = "") {
  if (obj === null || typeof obj !== "object" || Array.isArray(obj)) {
    throw new Error(`${filePath}: expected JSON object`);
  }
  for (const [k, v] of Object.entries(obj)) {
    const parts = [...prefix, k];
    const joined = parts.join(".");
    if (v !== null && typeof v === "object" && !Array.isArray(v)) {
      flattenJson(v, parts, out, filePath);
    } else {
      if (out.has(joined)) throw new Error(`${filePath}: duplicate key "${joined}"`);
      out.set(joined, String(v ?? ""));
    }
  }
  return out;
}

function inferLocaleFromPath(filePath) {
  const base = path.basename(filePath).replace(/\.(ya?ml|json)$/i, "");
  const dot = base.lastIndexOf(".");
  return dot === -1 ? base : base.slice(dot + 1);
}

function loadLocales() {
  const files = listFiles(LOCALES_DIR, [".yml", ".yaml", ".json"]);
  if (files.length === 0) {
    throw new Error(`no locale files found under ${LOCALES_DIR}`);
  }
  /** @type {Map<string, Map<string, string>>} */
  const byLocale = new Map();

  for (const file of files) {
    if (file.endsWith(".json")) {
      const parsed = JSON.parse(fs.readFileSync(file, "utf8"));
      const version = Number(parsed._version ?? 1);
      delete parsed._version;
      const entries = flattenJson(parsed, [], new Map(), file);
      if (version === 2) {
        for (const [full, value] of entries) {
          const lastDot = full.lastIndexOf(".");
          if (lastDot === -1) throw new Error(`${file}: _version: 2 key "${full}" missing locale`);
          const key = full.slice(0, lastDot);
          const locale = full.slice(lastDot + 1);
          if (!byLocale.has(locale)) byLocale.set(locale, new Map());
          if (byLocale.get(locale).has(key)) throw new Error(`${file}: duplicate key "${key}" for "${locale}"`);
          byLocale.get(locale).set(key, value);
        }
      } else {
        const locale = inferLocaleFromPath(file);
        if (!byLocale.has(locale)) byLocale.set(locale, new Map());
        for (const [key, value] of entries) {
          if (byLocale.get(locale).has(key)) throw new Error(`${file}: duplicate key "${key}" for "${locale}"`);
          byLocale.get(locale).set(key, value);
        }
      }
      continue;
    }

    const { version, entries } = parseYamlFlat(file);
    if (version === 2) {
      for (const [full, value] of entries) {
        const lastDot = full.lastIndexOf(".");
        if (lastDot === -1) throw new Error(`${file}: _version: 2 key "${full}" missing locale`);
        const key = full.slice(0, lastDot);
        const locale = full.slice(lastDot + 1);
        if (!byLocale.has(locale)) byLocale.set(locale, new Map());
        if (byLocale.get(locale).has(key)) throw new Error(`${file}: duplicate key "${key}" for "${locale}"`);
        byLocale.get(locale).set(key, value);
      }
    } else {
      const locale = inferLocaleFromPath(file);
      if (!byLocale.has(locale)) byLocale.set(locale, new Map());
      for (const [key, value] of entries) {
        if (byLocale.get(locale).has(key)) throw new Error(`${file}: duplicate key "${key}" for "${locale}"`);
        byLocale.get(locale).set(key, value);
      }
    }
  }

  return byLocale;
}

function extractParams(text) {
  const params = new Set();
  const re = /%\{([A-Za-z0-9_]+)\}/g;
  let match;
  while ((match = re.exec(text)) !== null) {
    params.add(match[1]);
  }
  return [...params].sort();
}

function renderGeneratedTs(byLocale) {
  const locales = [...byLocale.keys()].sort();
  const baseMap = byLocale.get(BASE_LOCALE) ?? byLocale.get(locales[0]);
  const keys = [...baseMap.keys()].sort();

  const lines = [
    "// Generated by scripts/i18n.mjs — do not edit manually.",
    "// Run `node scripts/i18n.mjs generate` after updating `locales/*`.",
    "",
    `export type AppLocale = ${locales.map((l) => JSON.stringify(l)).join(" | ")};`,
    "",
    "export interface TranslationParams {",
  ];

  for (const key of keys) {
    const params = extractParams(baseMap.get(key) ?? "");
    if (params.length === 0) {
      lines.push(`  ${JSON.stringify(key)}: Record<never, never>;`);
    } else {
      const fields = params.map((p) => `${p}: string | number`).join("; ");
      lines.push(`  ${JSON.stringify(key)}: { ${fields} };`);
    }
  }

  lines.push(
    "}",
    "",
    "export type TranslationKey = keyof TranslationParams;",
    "",
    "export type NoParamTranslationKey = {",
    "  [K in TranslationKey]: keyof TranslationParams[K] extends never ? K : never;",
    "}[TranslationKey];",
    "",
  );
  return lines.join("\n");
}

function appSourceFiles() {
  const candidates = [path.join(ROOT, "App.tsx"), ...listFiles(path.join(ROOT, "src"), [".ts", ".tsx"])];
  return candidates.filter(
    (f) =>
      fs.existsSync(f) &&
      f !== GENERATED_PATH &&
      !f.endsWith(".test.ts") &&
      !f.endsWith(".test.tsx"),
  );
}

function scanSources(baseKeys) {
  const files = appSourceFiles();
  const seenLiterals = new Set();
  const templatePrefixes = new Set();

  for (const file of files) {
    const rel = path.relative(ROOT, file);
    const text = fs.readFileSync(file, "utf8");

    if (file !== ALLOWED_NATIVE_IMPORT_FILE) {
      const importRe = /from\s+["']nsh\/i18n["']/g;
      if (importRe.test(text)) {
        fail(`${rel}: import from "../i18n" (or "../../i18n") instead of "nsh/i18n" so t() stays type-checked`);
      }
    }

    // Direct t("...") or t('...') calls
    const callRe = /\bt\(\s*(["'])([^"']+)\1/g;
    let m;
    while ((m = callRe.exec(text)) !== null) {
      const key = m[2];
      if (!baseKeys.has(key)) {
        fail(`${rel}: unknown translation key ${JSON.stringify(key)}`);
      }
      seenLiterals.add(key);
    }

    // Template literal calls like t(`player.quality.${id}`)
    const tplCallRe = /\bt\(\s*`([^`$]+)\$\{/g;
    while ((m = tplCallRe.exec(text)) !== null) {
      const prefix = m[1];
      const matchesAny = [...baseKeys].some((k) => k.startsWith(prefix));
      if (!matchesAny) {
        fail(`${rel}: template translation prefix "${prefix}*" matches no key in locales/${BASE_LOCALE}.yml`);
      }
      templatePrefixes.add(prefix);
    }

    // Collect all string literals in app source (covers lookup tables like REGION_LABEL_KEY / titleKey)
    const strRe = /(["'])([A-Za-z][A-Za-z0-9_.-]*\.[A-Za-z0-9_.-]+)\1/g;
    while ((m = strRe.exec(text)) !== null) {
      seenLiterals.add(m[2]);
    }
  }

  for (const key of baseKeys) {
    const usedByLiteral = seenLiterals.has(key);
    const usedByTemplate = [...templatePrefixes].some((prefix) => key.startsWith(prefix));
    if (!usedByLiteral && !usedByTemplate) {
      fail(`unused translation key "${key}" (present in locales/ but not referenced in App.tsx or src/)`);
    }
  }
}

function runGenerate() {
  const byLocale = loadLocales();
  const next = renderGeneratedTs(byLocale);
  fs.mkdirSync(path.dirname(GENERATED_PATH), { recursive: true });
  fs.writeFileSync(GENERATED_PATH, next, "utf8");
  const baseMap = byLocale.get(BASE_LOCALE);
  console.log(`Generated ${path.relative(ROOT, GENERATED_PATH)} (${byLocale.size} locales, ${baseMap?.size ?? 0} keys).`);
}

function runCheck() {
  const byLocale = loadLocales();
  const locales = [...byLocale.keys()].sort();
  if (!byLocale.has(BASE_LOCALE)) {
    fail(`missing base locale "${BASE_LOCALE}" in locales/`);
    return;
  }

  const baseMap = byLocale.get(BASE_LOCALE);
  const baseKeys = new Set(baseMap.keys());

  for (const locale of locales) {
    const map = byLocale.get(locale);
    for (const [key, value] of map) {
      if (value.trim() === "") {
        fail(`locales/${locale}: key "${key}" has an empty value`);
      }
    }
    if (locale === BASE_LOCALE) continue;

    for (const key of baseMap.keys()) {
      if (!map.has(key)) {
        fail(`locales/${locale}: missing key "${key}" (present in ${BASE_LOCALE})`);
      } else {
        const expected = extractParams(baseMap.get(key));
        const actual = extractParams(map.get(key));
        if (JSON.stringify(expected) !== JSON.stringify(actual)) {
          fail(
            `locales/${locale}: placeholder mismatch for "${key}": expected [${expected.join(", ")}], got [${actual.join(", ")}]`,
          );
        }
      }
    }

    for (const key of map.keys()) {
      if (!baseMap.has(key)) {
        fail(`locales/${locale}: extra key "${key}" not present in ${BASE_LOCALE}`);
      }
    }
  }

  const expectedGenerated = renderGeneratedTs(byLocale);
  if (!fs.existsSync(GENERATED_PATH)) {
    fail(`${path.relative(ROOT, GENERATED_PATH)} is missing; run \`node scripts/i18n.mjs generate\``);
  } else {
    const actualGenerated = fs.readFileSync(GENERATED_PATH, "utf8");
    if (actualGenerated !== expectedGenerated) {
      fail(`${path.relative(ROOT, GENERATED_PATH)} is out of date; run \`node scripts/i18n.mjs generate\``);
    }
  }

  scanSources(baseKeys);

  if (!process.exitCode) {
    console.log(`i18n check passed (${locales.join(", ")}; ${baseKeys.size} keys verified).`);
  }
}

const cmd = process.argv[2] ?? "check";
try {
  if (cmd === "generate") runGenerate();
  else if (cmd === "check") runCheck();
  else {
    console.error("Usage: node scripts/i18n.mjs <generate|check>");
    process.exitCode = 1;
  }
} catch (error) {
  fail(error instanceof Error ? error.message : String(error));
}
