/** Pencereye bırakılan yolların ayrımı. */
export interface DroppedPaths {
  /** Desteklenen ses dosyaları (bırakılış sırasıyla). */
  tracks: string[];
  /** Geri kalanlar: klasör oldukları varsayılır ve kütüphaneye eklenir. */
  folders: string[];
}

/** Yolun uzantısı (küçük harfle, noktasız); uzantı yoksa boş metin. */
function extensionOf(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? "";
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/**
 * Bırakılan yolları ses dosyaları ve klasörler olarak ayırır. Desteklenen uzantı
 * listesi henüz bilinmiyorsa (program açılırken) hepsi ses dosyası sayılır.
 */
export function splitDropped(paths: string[], extensions: string[]): DroppedPaths {
  if (extensions.length === 0) return { tracks: [...paths], folders: [] };
  const supported = new Set(extensions.map((e) => e.toLowerCase()));
  const result: DroppedPaths = { tracks: [], folders: [] };
  for (const path of paths) {
    (supported.has(extensionOf(path)) ? result.tracks : result.folders).push(path);
  }
  return result;
}
