// node_modules altındaki bütün paketlerin lisanslarını denetler.
// Kural: AGPL lisanslı ya da ticari kullanımı yasaklayan paket kullanılmaz. Lisansı
// belirtilmemiş ya da "UNLICENSED" (izin verilmemiş, tescilli) paket de kabul edilmez:
// lisans eklemeden önce doğrulanır (CLAUDE.md).
// Kullanım: npm run check:licenses

import { readdirSync, readFileSync, existsSync } from "node:fs";
import { join } from "node:path";

/** Yasak lisans kalıpları (büyük/küçük harf duyarsız). */
const FORBIDDEN = [
  /AGPL/i,
  /SSPL/i,
  /BUSL/i,
  /Business Source/i,
  /Commons Clause/i,
  /Elastic-2\.0/i,
  /CC-BY-NC/i,
  /NonCommercial/i,
  /PolyForm-Noncommercial/i,
  /Prosperity/i,
  /^UNLICENSED$/i,
];

function licenseOf(pkg) {
  const value = pkg.license ?? pkg.licenses;
  if (typeof value === "string") return value;
  if (Array.isArray(value)) return value.map((l) => l.type ?? l).join(" OR ");
  if (value && typeof value === "object") return value.type ?? "BİLİNMİYOR";
  return "BİLİNMİYOR";
}

function* packageDirs(root) {
  if (!existsSync(root)) return;
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    if (!entry.isDirectory() || entry.name.startsWith(".")) continue;
    const dir = join(root, entry.name);
    if (entry.name.startsWith("@")) {
      yield* packageDirs(dir);
      continue;
    }
    yield dir;
    yield* packageDirs(join(dir, "node_modules"));
  }
}

const problems = [];
const unknown = [];
let count = 0;
for (const dir of packageDirs("node_modules")) {
  const file = join(dir, "package.json");
  if (!existsSync(file)) continue;
  const pkg = JSON.parse(readFileSync(file, "utf8"));
  if (!pkg.name) continue;
  count++;
  const license = licenseOf(pkg);
  // "A OR B" biçiminde, yasak olmayan bir seçenek varsa kabul edilir.
  const options = license.replace(/[()]/g, "").split(/\s+OR\s+/i);
  if (options.every((option) => FORBIDDEN.some((re) => re.test(option)))) {
    problems.push(`${pkg.name}@${pkg.version}: ${license}`);
  } else if (license === "BİLİNMİYOR" || /^SEE LICENSE IN/i.test(license)) {
    unknown.push(`${pkg.name}@${pkg.version}`);
  }
}

if (unknown.length > 0) {
  console.error(`Lisansı belirtilmemiş ${unknown.length} paket:\n  ${unknown.join("\n  ")}`);
}
if (problems.length > 0) {
  console.error(`Yasak lisanslı paketler bulundu:\n  ${problems.join("\n  ")}`);
}
if (problems.length > 0 || unknown.length > 0) {
  process.exit(1);
}
console.log(`Lisans denetimi tamam: ${count} paket incelendi, yasak lisans yok.`);
