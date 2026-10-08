// Arayüz ile çekirdek arasındaki komut sözleşmesini denetler:
//
// - Arayüzün `invoke("ad", …)` ile çağırdığı her komut çekirdekte tanımlı ve
//   `generate_handler!` listesinde kayıtlı mı? (Değilse çağrı çalışma anında başarısız olur.)
// - Kayıtlı her komutu arayüz kullanıyor mu? (Kullanılmayan komut ölü koddur.)
// - Gönderilen argüman adları Rust'taki parametre adlarıyla eşleşiyor mu? Tauri, Rust'taki
//   `snake_case` parametreyi arayüzde `camelCase` bekler. Zorunlu (Option olmayan) her
//   parametre gönderilmeli; Rust'ta olmayan bir ad gönderilmemeli. Yanlış ad, hatası
//   ancak o düğmeye basılınca görülen sessiz bir arızadır.
//
// Kullanım: npm run check:commands

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const RUST_COMMANDS = "src-tauri/src/commands.rs";
const RUST_SETUP = "src-tauri/src/lib.rs";
const UI_ROOT = "src";

/** Tauri'nin kendisinin verdiği parametreler (arayüzden gönderilmez). */
const INJECTED = /^(tauri::)?(State|AppHandle|Window|WebviewWindow)\b/;

const camel = (name) => name.replace(/_([a-z0-9])/g, (_, c) => c.toUpperCase());

/** `#[tauri::command]` işaretli fonksiyonlar ve parametreleri. */
function rustCommands(source) {
  const commands = new Map();
  const pattern = /#\[tauri::command\]\s*pub\s+(?:async\s+)?fn\s+([a-z0-9_]+)\s*\(([^)]*)\)/g;
  for (const [, name, params] of source.matchAll(pattern)) {
    const args = [];
    // Parametreler virgülle ayrılır; tür içindeki `<…, …>` virgüllerine dikkat.
    let depth = 0;
    let current = "";
    for (const char of params) {
      if (char === "<") depth++;
      if (char === ">") depth--;
      if (char === "," && depth === 0) {
        args.push(current);
        current = "";
      } else current += char;
    }
    args.push(current);
    const parsed = args
      .map((arg) => arg.trim())
      .filter(Boolean)
      .map((arg) => {
        const colon = arg.indexOf(":");
        return { name: arg.slice(0, colon).trim(), type: arg.slice(colon + 1).trim() };
      })
      .filter((arg) => !INJECTED.test(arg.type))
      .map((arg) => ({ key: camel(arg.name), optional: /^Option</.test(arg.type) }));
    commands.set(name, parsed);
  }
  return commands;
}

/** `generate_handler![ … ]` listesindeki komut adları. */
function registered(source) {
  const block = source.match(/generate_handler!\[([^\]]*)\]/);
  if (!block) throw new Error(`${RUST_SETUP}: generate_handler! bulunamadı`);
  return new Set([...block[1].matchAll(/commands::([a-z0-9_]+)/g)].map((m) => m[1]));
}

function* sourceFiles(dir) {
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) yield* sourceFiles(path);
    else if (/\.(ts|tsx)$/.test(entry) && !/\.test\.tsx?$/.test(entry)) yield path;
  }
}

/** Arayüzdeki `invoke("ad", { … })` çağrıları ve gönderilen argüman adları. */
function uiCalls() {
  const calls = [];
  const pattern = /invoke(?:<[^(]*>)?\(\s*"([a-z0-9_]+)"\s*(?:,\s*\{([^}]*)\})?/g;
  for (const file of sourceFiles(UI_ROOT)) {
    const text = readFileSync(file, "utf8");
    for (const match of text.matchAll(pattern)) {
      const keys = (match[2] ?? "")
        .split(",")
        .map((part) => part.split(":")[0].trim())
        .filter(Boolean);
      const line = text.slice(0, match.index).split("\n").length;
      calls.push({ name: match[1], keys, where: `${file}:${line}` });
    }
  }
  return calls;
}

const commands = rustCommands(readFileSync(RUST_COMMANDS, "utf8"));
const handlers = registered(readFileSync(RUST_SETUP, "utf8"));
const calls = uiCalls();
const problems = [];

for (const name of handlers) {
  if (!commands.has(name)) problems.push(`${name}: kayıtlı ama ${RUST_COMMANDS} içinde yok`);
}
for (const name of commands.keys()) {
  if (!handlers.has(name)) problems.push(`${name}: tanımlı ama generate_handler! listesinde yok`);
  if (!calls.some((call) => call.name === name)) {
    problems.push(`${name}: çekirdekte var ama arayüz hiç çağırmıyor (ölü kod)`);
  }
}
for (const call of calls) {
  const params = commands.get(call.name);
  if (!params) {
    problems.push(`${call.where}: "${call.name}" komutu çekirdekte yok`);
    continue;
  }
  for (const key of call.keys) {
    if (!params.some((p) => p.key === key)) {
      problems.push(`${call.where}: "${call.name}" komutunda "${key}" diye bir parametre yok`);
    }
  }
  for (const param of params) {
    if (!param.optional && !call.keys.includes(param.key)) {
      problems.push(`${call.where}: "${call.name}" komutuna zorunlu "${param.key}" gönderilmiyor`);
    }
  }
}

if (problems.length > 0) {
  console.error(`Komut sözleşmesi bozuk:\n  ${problems.join("\n  ")}`);
  process.exit(1);
}
console.log(
  `Komut sözleşmesi tamam: ${commands.size} komut, arayüzde ${calls.length} çağrı; adlar ve argümanlar eşleşiyor.`,
);
