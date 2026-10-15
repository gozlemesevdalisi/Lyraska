import { useCallback, useEffect, useRef, useState } from "react";
import {
  ABX_INACTIVE,
  abxAnswer,
  abxPause,
  abxSelect,
  abxStart,
  abxStatus,
  abxStop,
  pickAbxFile,
  type AbxChoice,
  type AbxSource,
  type AbxStatus,
} from "../lib/backend";
import { DEFAULT_CLIP_SECS, DEFAULT_START_SECS, DEFAULT_TRIALS } from "../lib/abx";

/** Test sürerken durumun (konum) sorulma aralığı. */
const POLL_MS = 250;

export interface AbxControls {
  status: AbxStatus;
  /** Kurulum: seçilen dosyalar ve ayarlar. */
  a: string | null;
  b: string | null;
  startSecs: number;
  clipSecs: number;
  trials: number;
  setStartSecs: (seconds: number) => void;
  setClipSecs: (seconds: number) => void;
  setTrials: (trials: number) => void;
  pickA: () => Promise<void>;
  pickB: () => Promise<void>;
  /** İki dosya çözülüp hizalanırken. */
  busy: boolean;
  error: string | null;
  start: () => Promise<void>;
  select: (choice: AbxChoice) => Promise<void>;
  pause: () => Promise<void>;
  answer: (guess: AbxSource) => Promise<void>;
  stop: () => Promise<void>;
}

function message(error: unknown): string {
  return typeof error === "string" ? error : error instanceof Error ? error.message : String(error);
}

/**
 * Kör dinleme testi. `visible`: panel açık mı; değilse süren test duraklatılır (ses arkada
 * çalmasın) ve durum sorulmaz. `onPlayerChange`: test başlayınca ya da kapanınca çağrılır;
 * çekirdek bu anlarda çalan şarkıyı kapatır ya da kaldığı yerde geri açar.
 */
export function useAbx(
  extensions: string[],
  visible: boolean,
  onPlayerChange?: () => void,
): AbxControls {
  const [status, setStatus] = useState<AbxStatus>(ABX_INACTIVE);
  const [a, setA] = useState<string | null>(null);
  const [b, setB] = useState<string | null>(null);
  const [startSecs, setStartSecs] = useState(DEFAULT_START_SECS);
  const [clipSecs, setClipSecs] = useState(DEFAULT_CLIP_SECS);
  const [trials, setTrials] = useState(DEFAULT_TRIALS);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);
  // İsteklerin sıra numarası: geç gelen eski yanıt (ör. yanıttan önce gönderilmiş bir durum
  // sorgusu) yenisinin üstüne yazıp denemeyi geri almasın.
  const issued = useRef(0);
  const applied = useRef(0);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const apply = useCallback((id: number, next: AbxStatus) => {
    if (!alive.current || id < applied.current) return;
    applied.current = id;
    setStatus(next);
    setError(next.error);
  }, []);
  const fail = useCallback((e: unknown) => {
    if (alive.current) setError(message(e));
  }, []);
  const run = useCallback(
    (action: () => Promise<AbxStatus>) => {
      const id = ++issued.current;
      return action()
        .then((next) => apply(id, next))
        .catch(fail);
    },
    [apply, fail],
  );

  // Test başlayınca ya da kapanınca (yanıtla, sorguyla ya da hatayla) oynatıcı da güncellensin.
  const playerChange = useRef(onPlayerChange);
  useEffect(() => {
    playerChange.current = onPlayerChange;
  }, [onPlayerChange]);
  const wasActive = useRef(status.active);
  useEffect(() => {
    if (wasActive.current === status.active) return;
    wasActive.current = status.active;
    playerChange.current?.();
  }, [status.active]);

  // Panel açılınca güncel durum (ör. başka yerden şarkı açıldıysa test kapanmıştır).
  useEffect(() => {
    if (visible) void run(abxStatus);
  }, [visible, run]);

  // Panelden ayrılınca ses arkada çalmasın.
  useEffect(() => {
    if (!visible && status.active && status.playing) void run(abxPause);
  }, [visible, status.active, status.playing, run]);

  // Test sürerken konum ve durum. Yanıt gelmeden yenisi gönderilmez (çekirdek meşgulken
  // sorgular birikmesin); geçici hata gösterilmez, bir sonraki turda yeniden denenir.
  useEffect(() => {
    if (!visible || !status.active) return;
    let inFlight = false;
    const timer = window.setInterval(() => {
      if (inFlight) return;
      inFlight = true;
      const id = ++issued.current;
      abxStatus()
        .then((next) => apply(id, next))
        .catch(() => {})
        .finally(() => {
          inFlight = false;
        });
    }, POLL_MS);
    return () => window.clearInterval(timer);
  }, [visible, status.active, apply]);

  const pick = useCallback(
    async (label: "A" | "B", set: (path: string) => void) => {
      try {
        const path = await pickAbxFile(label, extensions);
        if (path) set(path);
      } catch (e) {
        setError(message(e));
      }
    },
    [extensions],
  );

  const start = useCallback(async () => {
    if (!a || !b) return;
    setBusy(true);
    setError(null);
    await run(() => abxStart({ a, b, startSecs, clipSecs, trials }));
    if (alive.current) setBusy(false);
  }, [a, b, startSecs, clipSecs, trials, run]);

  return {
    status,
    a,
    b,
    startSecs,
    clipSecs,
    trials,
    setStartSecs,
    setClipSecs,
    setTrials,
    pickA: () => pick("A", setA),
    pickB: () => pick("B", setB),
    busy,
    error,
    start,
    select: (choice) => run(() => abxSelect(choice)),
    pause: () => run(abxPause),
    answer: (guess) => run(() => abxAnswer(guess)),
    stop: () => run(abxStop),
  };
}
