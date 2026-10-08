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

/**
 * Yakalanmamış arayüz hatalarını (istisnalar ve sonuçlanmayan sözler) çekirdeğin
 * hata günlüğüne yazar. Programın akışını değiştirmez.
 */
export function installErrorReporting(target: Window = window): () => void {
  const report = (error: unknown) => {
    logFrontendError(describeError(error)).catch(() => {
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
