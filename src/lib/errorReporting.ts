import { logFrontendError } from "./backend";

/** Günlüğe yazılacak hata metni: mesaj ve (varsa) yığın izinin ilk satırları. */
export function describeError(error: unknown): string {
  if (error instanceof Error) {
    const stack = (error.stack ?? "").split("\n").slice(1, 6).join("\n");
    return stack ? `${error.name}: ${error.message}\n${stack}` : `${error.name}: ${error.message}`;
  }
  if (typeof error === "string") return error;
  try {
    return JSON.stringify(error) ?? String(error);
  } catch {
    return String(error);
  }
}

/** Aynı hata en fazla bu kadar kez ayrıntısıyla yazılır. */
export const MAX_SAME_ERROR = 3;
/** Sonra her bu kadar tekrarda bir, tekrar sayısıyla tek satır yazılır. */
export const REPEAT_NOTE_EVERY = 100;
/** Bu kadar farklı hata görülünce sayaçlar sıfırlanır (bellek büyümesin). */
const MAX_DISTINCT = 200;

/**
 * Günlüğe yazılacak satırı seçer; yazılmayacaksa `null`. Her ekran karesinde atan bir hata
 * günlüğü saniyede 60 satırla doldurup açılış bilgisini ve asıl ilk nedeni birkaç saniyede
 * (1 MB sınırında) silerdi: aynı hatanın ilk birkaç tekrarı yazılır, sonrası sayılır.
 */
export function createRepeatFilter(): (message: string) => string | null {
  const counts = new Map<string, number>();
  return (message) => {
    if (counts.size >= MAX_DISTINCT && !counts.has(message)) counts.clear();
    const count = (counts.get(message) ?? 0) + 1;
    counts.set(message, count);
    if (count <= MAX_SAME_ERROR) return message;
    if (count % REPEAT_NOTE_EVERY === 0) {
      return `${message.split("\n")[0]} (aynı hata ${count}. kez)`;
    }
    return null;
  };
}

/**
 * Yakalanmamış arayüz hatalarını (istisnalar ve sonuçlanmayan sözler) çekirdeğin
 * hata günlüğüne yazar. Programın akışını değiştirmez.
 */
export function installErrorReporting(target: Window = window): () => void {
  const filter = createRepeatFilter();
  const report = (error: unknown) => {
    const line = filter(describeError(error));
    if (line === null) return;
    logFrontendError(line).catch(() => {
      /* Günlüğe yazılamazsa yapılacak bir şey yok. */
    });
  };
  const onError = (event: ErrorEvent) => report(event.error ?? event.message);
  const onRejection = (event: PromiseRejectionEvent) => report(event.reason);
  target.addEventListener("error", onError);
  target.addEventListener("unhandledrejection", onRejection);
  return () => {
    target.removeEventListener("error", onError);
    target.removeEventListener("unhandledrejection", onRejection);
  };
}
