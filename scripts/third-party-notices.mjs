// Programla dağıtılan üçüncü taraf kodun lisans bildirimlerini üretir:
// public/lisanslar/UCUNCU-TARAF.txt (programın içine paketlenir, Ayarlar > Program'dan açılır).
//
// Kaynaklar:
// - Rust: Windows hedefinde çalıştırılabilir dosyaya giren crate'ler (`cargo tree`, yalnızca normal
//   bağımlılıklar; derleme betikleri ve proc-macro'lar hariç). Lisans metinleri crate'in kendi
//   paketinden (~/.cargo/registry/cache/*/<ad>-<sürüm>.crate) okunur; önce `cargo fetch` gerekir.
// - npm: pakete giren (dev olmayan) bağımlılıklar (`npm ls --omit=dev`); lisans metinleri node_modules'tan.
//
// MIT/BSD/ISC gibi lisanslar ikili dağıtımda telif ve izin metninin verilmesini şart koşar
// (DENETIM.md C3). Çıktı belirlenimcidir (sıralı, tarih yok); CI yeniden üretip depodakiyle karşılaştırır.
// Kullanım: node scripts/third-party-notices.mjs [--check]

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir, homedir } from "node:os";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "..");
const OUTPUT = join(ROOT, "public", "lisanslar", "UCUNCU-TARAF.txt");
const TARGET = "x86_64-pc-windows-msvc";
const LICENSE_FILE = /^(LICEN[CS]E|COPYING|NOTICE)(\.|-|_|$)/i;

/** Yerel ayardan bağımsız sıralama (kod noktası): çıktı her makinede aynı olsun. */
function byCodePoint(a, b) {
  return a < b ? -1 : a > b ? 1 : 0;
}

/** Rust crate'leri: ad, sürüm, lisans ifadesi, depo. */
function rustPackages() {
  const out = execFileSync(
    "cargo",
    [
      "tree",
      "--locked",
      "--color",
      "never",
      "--target",
      TARGET,
      "-e",
      "normal,no-proc-macro",
      "--prefix",
      "none",
      "--format",
      "{p}|{l}|{r}",
    ],
    { cwd: join(ROOT, "src-tauri"), encoding: "utf8", maxBuffer: 64 << 20 },
  );
  const seen = new Map();
  for (const raw of out.split("\n")) {
    // CI `CARGO_TERM_COLOR=always` verse bile renk kodları ve "(*)" tekrar işareti temizlenir.
    const line = raw
      .replace(/\u001b\[[0-9;]*m/g, "")
      .replace(/ \(\*\)$/, "")
      .trim();
    if (!line) continue;
    const [pkg, license, repository] = line.split("|");
    const match = /^(\S+) v(\S+)(?: \((.*)\))?$/.exec(pkg);
    if (!match) throw new Error(`cargo tree satırı anlaşılamadı: ${line}`);
    const [, name, version, path] = match;
    if (name === "lyraska" || path) continue; // kendi kodumuz (yol bağımlılığı)
    seen.set(`${name} ${version}`, { name, version, license: license || "BİLİNMİYOR", repository });
  }
  return [...seen.values()].sort(
    (a, b) => byCodePoint(a.name, b.name) || byCodePoint(a.version, b.version),
  );
}

/** Crate paketindeki lisans dosyaları (ad → metin). */
function rustLicenseTexts(name, version) {
  const cargoHome = process.env.CARGO_HOME ?? join(homedir(), ".cargo");
  const cacheRoot = join(cargoHome, "registry", "cache");
  const file = `${name}-${version}.crate`;
  const archive = readdirSync(cacheRoot)
    .map((dir) => join(cacheRoot, dir, file))
    .find((p) => existsSync(p));
  if (!archive)
    throw new Error(
      `${file} bulunamadı; önce 'cd src-tauri && cargo fetch --locked --target ${TARGET}'`,
    );
  const listing = execFileSync("tar", ["-tzf", archive], { encoding: "utf8" })
    .split("\n")
    .filter((entry) => {
      const parts = entry.split("/");
      return parts.length === 2 && LICENSE_FILE.test(parts[1]);
    });
  if (listing.length === 0) return new Map();
  const dir = mkdtempSync(join(tmpdir(), "lyraska-lisans-"));
  try {
    execFileSync("tar", ["-xzf", archive, "-C", dir, ...listing]);
    return new Map(
      listing.map((entry) => [entry.split("/")[1], readFileSync(join(dir, entry), "utf8")]),
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** npm paketleri (pakete girenler): paket kilidinde `dev` olmayanlar; lisans dosyaları node_modules'tan. */
function npmPackages() {
  const lock = JSON.parse(readFileSync(join(ROOT, "package-lock.json"), "utf8"));
  const found = [];
  for (const [key, entry] of Object.entries(lock.packages ?? {})) {
    if (!key.startsWith("node_modules/") || entry.dev || entry.link) continue;
    const dir = join(ROOT, key);
    if (!existsSync(dir)) {
      if (entry.optional) continue; // bu platformda kurulmayan isteğe bağlı paket
      throw new Error(`${key} kurulu değil; önce 'npm ci'`);
    }
    const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    const license =
      typeof pkg.license === "string" ? pkg.license : (pkg.license?.type ?? "BİLİNMİYOR");
    const texts = new Map();
    for (const file of readdirSync(dir)) {
      if (LICENSE_FILE.test(file)) texts.set(file, readFileSync(join(dir, file), "utf8"));
    }
    const repository = typeof pkg.repository === "string" ? pkg.repository : pkg.repository?.url;
    found.push({
      name: pkg.name ?? key.slice("node_modules/".length),
      version: pkg.version,
      license,
      repository,
      texts,
    });
  }
  return found.sort((a, b) => byCodePoint(a.name, b.name) || byCodePoint(a.version, b.version));
}

function normalize(text) {
  return text
    .replace(/\r\n?/g, "\n")
    .replace(/[ \t]+\n/g, "\n")
    .trim();
}

/** Telif satırı: "Copyright (c) 2015 Ad", "Copyright 2016 The ...", "© 2020 ...", SPDX PackageCopyrightText. */
const COPYRIGHT =
  /\b[Cc]opyright\b\s*(?:\([Cc]\)|©)?\s*(?:\d{4}|[A-Z][\w.-]|<)|(?:\([Cc]\)|©)\s*\d{4}|PackageCopyrightText/;
/** Lisans metinlerinin kendi cümleleri (telif sahibi değil). */
const NOT_COPYRIGHT = /copyright (notice|holder|law|owner|statement|licen[cs]e|interest)/i;

/** Metnin telif satırları (lisans metninden ayrı listelenir). */
function copyrightLines(text) {
  const lines = new Set();
  for (const line of normalize(text).split("\n")) {
    if (COPYRIGHT.test(line) && !NOT_COPYRIGHT.test(line)) lines.add(line.trim());
  }
  return [...lines];
}

function build() {
  const texts = new Map(); // hash → { text, users: [] }
  const addText = (text, user) => {
    const clean = normalize(text);
    const hash = createHash("sha256").update(clean).digest("hex").slice(0, 12);
    if (!texts.has(hash)) texts.set(hash, { text: clean, users: [] });
    texts.get(hash).users.push(user);
    return hash;
  };

  const rust = rustPackages().map((p) => ({ ...p, texts: rustLicenseTexts(p.name, p.version) }));
  const npm = npmPackages();

  const lines = [];
  lines.push("LYRASKA — ÜÇÜNCÜ TARAF LİSANS BİLDİRİMLERİ");
  lines.push("");
  lines.push(
    "Lyraska GPL-3.0-only lisansıyla dağıtılır (kaynak kodu: https://github.com/gozlemesevdalisi/Lyraska).",
  );
  lines.push(
    "Program aşağıdaki açık kaynak kütüphaneleri içerir. Her kütüphanenin lisansı ve telif sahipleri",
  );
  lines.push("burada; lisans metinlerinin tamamı sonda (aynı metni kullananlar tek kez verilir).");
  lines.push(
    "Bu dosya scripts/third-party-notices.mjs ile üretilir ve CI'da depodakiyle karşılaştırılır.",
  );
  lines.push("");

  const section = (title, packages) => {
    lines.push("=".repeat(78));
    lines.push(title);
    lines.push("=".repeat(78));
    lines.push("");
    for (const p of packages) {
      lines.push(`${p.name} ${p.version}`);
      lines.push(`  Lisans: ${p.license}`);
      if (p.repository)
        lines.push(`  Kaynak: ${p.repository.replace(/^git\+/, "").replace(/\.git$/, "")}`);
      const copyrights = new Set();
      const refs = [];
      for (const [file, text] of [...p.texts.entries()].sort()) {
        for (const c of copyrightLines(text)) copyrights.add(c);
        refs.push(`${file} → metin ${addText(text, `${p.name} ${p.version}`)}`);
      }
      for (const c of copyrights) lines.push(`  ${c}`);
      if (refs.length === 0) {
        // Paket lisans dosyası taşımıyor (çoğu zaman aynı deponun başka paketinde): oraya yollanır.
        const sibling = packages.find(
          (o) => o !== p && o.repository && o.repository === p.repository && o.texts.size > 0,
        );
        lines.push(
          sibling
            ? `  (pakette lisans dosyası yok; aynı deponun ${sibling.name} ${sibling.version} paketindeki metin geçerli)`
            : "  (pakette lisans dosyası yok; lisans ifadesi yukarıda, metin için kaynak deposuna bakın)",
        );
      }
      for (const r of refs) lines.push(`  ${r}`);
      lines.push("");
    }
  };
  section(`RUST KÜTÜPHANELERİ (${rust.length}; ses motoru, analiz, kütüphane)`, rust);
  section(`JAVASCRIPT KÜTÜPHANELERİ (${npm.length}; arayüz)`, npm);

  lines.push("=".repeat(78));
  lines.push(`LİSANS METİNLERİ (${texts.size} farklı metin)`);
  lines.push("=".repeat(78));
  lines.push("");
  for (const [hash, { text, users }] of [...texts.entries()].sort()) {
    lines.push("-".repeat(78));
    lines.push(`metin ${hash} — kullananlar: ${[...new Set(users)].join(", ")}`);
    lines.push("-".repeat(78));
    lines.push(text);
    lines.push("");
  }
  return lines.join("\n") + "\n";
}

const content = build();
if (process.argv.includes("--check")) {
  const current = existsSync(OUTPUT) ? readFileSync(OUTPUT, "utf8") : "";
  if (current !== content) {
    console.error(
      `${OUTPUT} güncel değil. 'node scripts/third-party-notices.mjs' ile üretip commit edin.`,
    );
    // İlk farkı göster: CI günlüğünden neden anlaşılsın (makineye bağlı bir fark varsa yeri belli olsun).
    const have = current.split("\n");
    const want = content.split("\n");
    const limit = Math.min(have.length, want.length);
    let i = 0;
    while (i < limit && have[i] === want[i]) i += 1;
    console.error(
      `Satır sayısı: depoda ${have.length}, üretilen ${want.length}; ilk fark ${i + 1}. satırda:`,
    );
    for (let k = Math.max(0, i - 2); k < Math.min(limit, i + 6); k += 1) {
      console.error(`  depoda   ${k + 1}: ${have[k] ?? ""}`);
      console.error(`  üretilen ${k + 1}: ${want[k] ?? ""}`);
    }
    process.exit(1);
  }
  console.log("Üçüncü taraf lisans bildirimleri güncel.");
} else {
  writeFileSync(OUTPUT, content);
  const size = (Buffer.byteLength(content) / 1024).toFixed(0);
  console.log(`Yazıldı: ${OUTPUT} (${size} KB)`);
}
