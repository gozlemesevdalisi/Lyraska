/**
 * Çalma sırası: kütüphaneden bir şarkı çalınınca, o anki liste sıra olur.
 * Şarkı bitince ya da "sonraki"ye basılınca sıradaki çalar.
 */
export interface Queue {
  paths: string[];
  index: number;
}

export const EMPTY_QUEUE: Queue = { paths: [], index: -1 };

/** Listeden bir şarkı seçilince yeni sıra. */
export function queueFrom(paths: string[], path: string): Queue {
  const index = paths.indexOf(path);
  return index < 0 ? { paths: [path], index: 0 } : { paths, index };
}

/** Sonraki şarkı; sıra bittiyse `null`. */
export function nextInQueue(queue: Queue): Queue | null {
  return queue.index + 1 < queue.paths.length ? { ...queue, index: queue.index + 1 } : null;
}

/** Önceki şarkı; başta ise `null`. */
export function previousInQueue(queue: Queue): Queue | null {
  return queue.index > 0 ? { ...queue, index: queue.index - 1 } : null;
}

export function currentPath(queue: Queue): string | null {
  return queue.paths[queue.index] ?? null;
}

/** "Önceki" düğmesi: şarkının ilk 3 saniyesinden sonra basılırsa şarkı başa sarılır. */
export const RESTART_THRESHOLD_SECONDS = 3;
