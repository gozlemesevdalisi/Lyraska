/** Kütüphanedeki şarkı haritası (arka plan analizi) bilgilerinin gösterimi. */

const BPM = new Intl.NumberFormat("tr-TR", { maximumFractionDigits: 0 });

/**
 * Listedeki BPM sütunu: tempo yuvarlanır; analiz edildiği hâlde ritmi olmayan
 * şarkıda "—", henüz analiz edilmemişte boş.
 */
export function bpmLabel(track: { bpm: number | null; analyzed: boolean }): string {
  if (track.bpm !== null && Number.isFinite(track.bpm)) return BPM.format(track.bpm);
  return track.analyzed ? "—" : "";
}

/**
 * Analiz ilerledikçe listenin (BPM sütunu) ne zaman tazeleneceği: her şarkıda
 * değil, kütüphanenin yaklaşık %2'si analiz edildikçe ve analiz bitince. Büyük
 * kütüphanede liste her birkaç saniyede baştan okunmasın diye.
 */
export function analysisRefreshKey(analyzed: number, total: number): number {
  if (total <= 0 || analyzed >= total) return -1;
  const step = Math.max(1, Math.ceil(total / 50));
  return Math.floor(analyzed / step);
}

/** Analiz sürüyor mu (bekleyen şarkı var mı)? */
export function analysisPending(analyzed: number, total: number): boolean {
  return total > 0 && analyzed < total;
}
