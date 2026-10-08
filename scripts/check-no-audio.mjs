// Depoda ses dosyası olmadığını denetler (telif): yalnızca projenin kendi ürettiği
// sentetik test sesleri (src-tauri/tests/data/) izinlidir. CI'da çalışır.
import { execFileSync } from "node:child_process";

const AUDIO =
  /\.(mp3|flac|wav|wave|aif|aiff|aifc|ogg|oga|opus|m4a|m4b|m4p|mp4|aac|caf|mka|webm|wma|ape|wv|dsf|dff|mid|midi)$/i;
const ALLOWED_DIR = "src-tauri/tests/data/";

const files = execFileSync("git", ["ls-files", "-z"], { encoding: "utf8" })
  .split("\0")
  .filter(Boolean);
const offending = files.filter((f) => AUDIO.test(f) && !f.startsWith(ALLOWED_DIR));

if (offending.length > 0) {
  console.error("Depoda ses dosyası var (telif nedeniyle yasak):");
  for (const f of offending) console.error(`  ${f}`);
  console.error(`Yalnızca projenin ürettiği sentetik sesler ${ALLOWED_DIR} altında durabilir.`);
  process.exit(1);
}
console.log(`Ses dosyası denetimi tamam: ${files.length} dosya incelendi.`);
