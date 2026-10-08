/**
 * Kapak resmi olmayan şarkılar için özgün renk kapağı: albüm (ya da sanatçı) adından
 * her seferinde aynı çıkan iki renkli bir geçiş. Aynı albümün şarkıları aynı kapağı alır.
 */
export function coverGradient(seed: string): string {
  // FNV-1a: kısa metinlerde de iyi dağılan, basit bir özet.
  let hash = 0x811c9dc5;
  for (const char of seed) {
    hash ^= char.codePointAt(0)!;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  const hue = hash % 360;
  const second = (hue + 35 + ((hash >>> 9) % 70)) % 360;
  return `linear-gradient(135deg, hsl(${hue} 78% 62%), hsl(${second} 66% 44%))`;
}
