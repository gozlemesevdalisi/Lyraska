import type { AbxResult, AbxStatus } from "./backend";
import { formatTime } from "./format";

/** Bölüm süresi seçenekleri (saniye) ve deneme sayısı seçenekleri. */
export const CLIP_CHOICES = [10, 20, 30] as const;
export const TRIAL_CHOICES = [8, 12, 16, 20] as const;
export const DEFAULT_CLIP_SECS = 20;
export const DEFAULT_TRIALS = 16;
export const DEFAULT_START_SECS = 30;

/** Şansla bu olasılığın altında kalan sonuç "farkı duyuyor" sayılır (yaygın ölçüt). */
export const SIGNIFICANCE = 0.05;

/**
 * Rastgele tahminle `total` denemenin en az `correct` tanesini bilme olasılığı (tek yönlü binom
 * sınaması). Çekirdekteki hesabın aynısı; arayüz "kaç doğru gerekir" bilgisini bununla gösterir.
 */
export function binomialTail(correct: number, total: number): number {
  if (correct <= 0) return 1;
  if (correct > total) return 0;
  let term = 0.5 ** total;
  let tail = 0;
  for (let k = 0; k <= total; k++) {
    if (k >= correct) tail += term;
    term *= (total - k) / (k + 1);
  }
  return Math.min(tail, 1);
}

/** Şansla açıklanamayacak sonuç için gereken en az doğru sayısı (ör. 16 denemede 12). */
export function neededCorrect(total: number): number {
  for (let k = 0; k <= total; k++) if (binomialTail(k, total) <= SIGNIFICANCE) return k;
  return total + 1;
}

const PERCENT = new Intl.NumberFormat("tr-TR", { maximumFractionDigits: 1 });

/** Olasılığın yüzde metni: 0,0384 → "%3,8"; binde birden küçükse "%0,1'den az". */
export function percentText(p: number): string {
  if (p < 0.001) return "%0,1'den az";
  return `%${PERCENT.format(p * 100)}`;
}

/** Sonucu sade Türkçeyle anlatır. */
export function verdict(result: AbxResult): { heard: boolean; title: string; detail: string } {
  const heard = result.pValue <= SIGNIFICANCE;
  const chance = percentText(result.pValue);
  const base = `${result.total} denemede ${result.correct} doğru. Bu sonucun yalnızca şansla çıkma olasılığı ${chance}.`;
  return heard
    ? {
        heard,
        title: "Farkı duyuyorsunuz",
        detail: `${base} Bu çok düşük: bu bölümde iki sürümü gerçekten ayırt ediyorsunuz.`,
      }
    : {
        heard,
        title: "Ayırt edilemedi",
        detail: `${base} Bu sonuç tahminle de kolayca çıkabilir: bu bölümde iki sürüm kulağınıza aynı geliyor.`,
      };
}

const DB = new Intl.NumberFormat("tr-TR", { maximumFractionDigits: 1, minimumFractionDigits: 1 });

/**
 * Kurulumun özeti: "00:30–00:50 · seviye farkı 0,4 dB eşitlendi · kayma 25,1 ms düzeltildi".
 * Küçük farklar (0,05 dB, 0,05 ms altı) yazılmaz.
 */
export function setupSummary(status: AbxStatus): string {
  const parts = [
    `${formatTime(status.startSecs)}–${formatTime(status.startSecs + status.clipSecs)}`,
  ];
  const level = Math.abs(status.levelDiffDb);
  if (level >= 0.05) parts.push(`seviye farkı ${DB.format(level)} dB eşitlendi`);
  const offset = Math.abs(status.offsetMs);
  if (offset >= 0.05) parts.push(`kayma ${DB.format(offset)} ms düzeltildi`);
  return parts.join(" · ");
}

/** Dosya yolunun son parçası (Windows ve diğer ayırıcılar). */
export function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}
