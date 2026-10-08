import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, getAudioDelay, getCalibrationTrack, setAudioDelay } from "../lib/backend";
import { positionAtEvent } from "../lib/marks";
import { clampAudioDelay, estimateAudioDelay, type DelayEstimate } from "../lib/sync";

/** Ayar kaydırılırken art arda gelen değişiklikler bu kadar bekleyip kaydedilir. */
const SAVE_DELAY_MS = 250;

export type Calibration =
  | { phase: "idle" }
  | { phase: "listening"; path: string; clicks: number[]; taps: number[] }
  | { phase: "done"; estimate: DelayEstimate; previousMs: number }
  | { phase: "failed"; message: string };

export interface SyncControls {
  /** Ses aygıtının ek gecikmesi (ms). */
  delayMs: number;
  setDelay: (ms: number) => void;
  calibration: Calibration;
  /** Tıklama kaydını çalar ve basışları dinlemeye başlar. */
  startCalibration: () => Promise<void>;
  /** Dinlemeyi bitirir ve sonucu uygular. */
  finishCalibration: () => void;
  /** Ölçümden önceki ayara döner. */
  undoCalibration: () => void;
  error: string | null;
}

/**
 * Ses–görüntü senkronu ayarı ve ölçümü. Ölçüm sırasında Boşluk tuşu oynatıcının
 * kısayolundan önce yakalanır (çal/duraklat yapmaz); kayıt bitince ya da her
 * tıklamaya basılınca sonuç kendiliğinden uygulanır.
 */
export function useSync(
  openPath: (path: string) => Promise<void>,
  positionNow: () => number,
  /** Sonuna kadar çalınıp biten kaydın yolu (yoksa `null`). */
  endedPath: string | null,
): SyncControls {
  const [delayMs, setDelayMs] = useState(0);
  /** Kaydedilecek değer (kaydırma bitince yazılır); `null`: kaydedilecek bir şey yok. */
  const [unsaved, setUnsaved] = useState<number | null>(null);
  const [calibration, setCalibration] = useState<Calibration>({ phase: "idle" });
  const [error, setError] = useState<string | null>(null);
  const calibrationRef = useRef(calibration);
  useEffect(() => {
    calibrationRef.current = calibration;
  }, [calibration]);

  useEffect(() => {
    let cancelled = false;
    getAudioDelay()
      .then((ms) => !cancelled && setDelayMs(ms))
      .catch((e) => !cancelled && setError(errorMessage(e)));
    return () => {
      cancelled = true;
    };
  }, []);

  // Ayar kaydırılırken her adım değil, son değer kaydedilir.
  useEffect(() => {
    if (unsaved === null) return;
    const timer = window.setTimeout(() => {
      setAudioDelay(unsaved)
        .then((applied) => {
          setDelayMs(applied);
          setError(null);
        })
        .catch((e) => setError(errorMessage(e)));
    }, SAVE_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [unsaved]);

  const setDelay = useCallback((ms: number) => {
    const value = clampAudioDelay(ms);
    setDelayMs(value);
    setUnsaved(value);
  }, []);

  /** Dinlemeyi bitirir: sonucu uygular ya da neden olmadığını yazar. */
  const finishWith = useCallback(
    (clicks: number[], taps: number[]) => {
      const estimate = estimateAudioDelay(taps, clicks);
      if (!estimate) {
        setCalibration({
          phase: "failed",
          message:
            "Yeterli tıklamaya basılmadı. Ölçümü yeniden başlatıp her tıklamada Boşluk'a basın.",
        });
        return;
      }
      setCalibration({ phase: "done", estimate, previousMs: delayMs });
      setDelay(estimate.delayMs);
    },
    [delayMs, setDelay],
  );

  // Tıklama kaydı sonuna kadar çalındıysa sonuç çıkarılır.
  if (calibration.phase === "listening" && endedPath !== null && endedPath === calibration.path) {
    finishWith(calibration.clicks, calibration.taps);
  }

  const startCalibration = useCallback(async () => {
    try {
      const track = await getCalibrationTrack();
      // Önce kayıt açılır (oynatıcı durumu güncellenir); eski "bitti" durumu yeni
      // ölçümü bitirmesin. İlk tıklamadan önce 2 sn sessizlik var.
      await openPath(track.path);
      setCalibration({ phase: "listening", path: track.path, clicks: track.clicks, taps: [] });
      setError(null);
    } catch (e) {
      setCalibration({ phase: "idle" });
      setError(errorMessage(e));
    }
  }, [openPath]);

  const finishCalibration = useCallback(() => {
    const current = calibrationRef.current;
    if (current.phase === "listening") finishWith(current.clicks, current.taps);
  }, [finishWith]);

  const undoCalibration = useCallback(() => {
    const current = calibrationRef.current;
    if (current.phase !== "done") return;
    setDelay(current.previousMs);
    setCalibration({ phase: "idle" });
  }, [setDelay]);

  // Ölçüm sırasında Boşluk: tıklamayı duyduğu an. Her tıklamaya basılınca ölçüm biter.
  const listening = calibration.phase === "listening";
  useEffect(() => {
    if (!listening) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.code !== "Space") return;
      // Yazı alanında Boşluk yazıdır, basış sayılmaz.
      const target = event.target instanceof Element ? event.target : null;
      if (target?.closest("input, textarea, select, [contenteditable]")) return;
      event.preventDefault();
      event.stopPropagation();
      const current = calibrationRef.current;
      if (event.repeat || current.phase !== "listening") return;
      const time = positionAtEvent(positionNow(), performance.now(), event.timeStamp);
      const taps = [...current.taps, time];
      const next = { ...current, taps };
      calibrationRef.current = next;
      if (taps.length >= current.clicks.length) finishWith(current.clicks, taps);
      else setCalibration(next);
    };
    window.addEventListener("keydown", onKey, { capture: true });
    return () => window.removeEventListener("keydown", onKey, { capture: true });
  }, [listening, positionNow, finishWith]);

  return {
    delayMs,
    setDelay,
    calibration,
    startCalibration,
    finishCalibration,
    undoCalibration,
    error,
  };
}
