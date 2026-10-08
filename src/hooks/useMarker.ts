import { useCallback, useEffect, useRef, useState } from "react";
import {
  errorMessage,
  evaluateAnnotation,
  getAnnotation,
  saveAnnotation,
  type BeatEvaluation,
  type SongMap,
} from "../lib/backend";
import {
  EMPTY_MARKS,
  addMark,
  marksFrom,
  positionAtEvent,
  undoMark,
  type MarkKind,
  type Marks,
} from "../lib/marks";

/** Değişiklikten bu kadar sonra kendiliğinden kaydedilir. */
const AUTOSAVE_MS = 800;
/**
 * Durmadan işaretlerken her vuruş kaydı erteler; yine de ilk kaydedilmemiş
 * değişiklikten en geç bu kadar sonra kaydedilir.
 */
const AUTOSAVE_MAX_WAIT_MS = 3000;

/** Kaydedilmemiş işaretler: hangi şarkının, hangi değişikliğe kadar. */
interface PendingMarks {
  path: string;
  beats: number[];
  drops: number[];
  change: number;
}

export interface MarkerControls {
  marks: Marks;
  recording: boolean;
  setRecording: (on: boolean) => void;
  /** Son vuruşun türü ve sırası (ekrandaki ışık için). */
  lastTap: { kind: MarkKind; id: number } | null;
  /** Kaydedilen dosya; kaydedilmemiş değişiklik varsa `null`. */
  savedFile: string | null;
  saving: boolean;
  evaluation: BeatEvaluation | null;
  evaluating: boolean;
  /** Programın çıkardığı şarkı haritası (bölümler, droplar); analiz bitmediyse `null`. */
  songMap: SongMap | null;
  error: string | null;
  mark: (kind: MarkKind) => void;
  undo: () => void;
  clear: () => void;
  evaluate: () => Promise<void>;
}

/**
 * İşaretleme aracı: şarkı çalarken Boşluk beat, D drop işaretler; Geri tuşu son
 * işareti geri alır, Esc işaretlemeyi bitirir. İşaretler değiştikçe kendiliğinden
 * kaydedilir (şarkı başına bir dosya; ses içermez).
 */
export function useMarker(
  trackPath: string | null,
  playing: boolean,
  positionNow: () => number,
  songMap: SongMap | null,
): MarkerControls {
  const [marks, setMarks] = useState<Marks>(EMPTY_MARKS);
  const [recording, setRecordingState] = useState(false);
  const [lastTap, setLastTap] = useState<MarkerControls["lastTap"]>(null);
  const [savedFile, setSavedFile] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [evaluation, setEvaluation] = useState<BeatEvaluation | null>(null);
  const [evaluating, setEvaluating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loadedPath, setLoadedPath] = useState<string | null>(null);
  const tapId = useRef(0);
  /** Her değişiklikte artar: kayıt sürerken yeni işaret geldiyse kirli kalınır. */
  const changeCount = useRef(0);
  /** İlk kaydedilmemiş değişikliğin zamanı (ms). */
  const dirtySince = useRef<number | null>(null);
  const pending = useRef<PendingMarks | null>(null);

  // Şarkı değişince o şarkının kayıtlı işaretleri yüklenir (öncekinin kaydedilmemiş
  // işaretleri aşağıda, şarkı değişimi etkisinde yazılır).
  if (loadedPath !== trackPath) {
    setLoadedPath(trackPath);
    setMarks(EMPTY_MARKS);
    setEvaluation(null);
    setSavedFile(null);
    setDirty(false);
    setError(null);
  }

  useEffect(() => {
    if (!trackPath) return;
    let cancelled = false;
    getAnnotation(trackPath)
      .then((saved) => {
        if (cancelled || !saved) return;
        setMarks((current) =>
          current.history.length === 0 ? marksFrom(saved.beats, saved.drops) : current,
        );
        setEvaluation(saved.evaluation);
        // Diskteki işaretler kayıtlı sayılır: değiştirmeden de ölçülebilir.
        setSavedFile((current) => current ?? "kayıtlı");
      })
      .catch((e) => !cancelled && setError(errorMessage(e)));
    return () => {
      cancelled = true;
    };
  }, [trackPath]);

  // Kaydedilmemiş son durum: zamanlayıcı ve şarkı değişimi buradan yazar.
  useEffect(() => {
    pending.current =
      dirty && trackPath
        ? { path: trackPath, beats: marks.beats, drops: marks.drops, change: changeCount.current }
        : null;
  }, [dirty, marks, trackPath]);

  // Şarkı değişince (boşluksuz geçiş, "Sonraki") kaydedilmemiş işaretler önceki
  // şarkının dosyasına hemen yazılır; yoksa şarkının son vuruşları kaybolurdu.
  useEffect(
    () => () => {
      const unsaved = pending.current;
      dirtySince.current = null;
      if (!unsaved) return;
      pending.current = null;
      saveAnnotation(unsaved.path, unsaved.beats, unsaved.drops).catch((e) =>
        setError(errorMessage(e)),
      );
    },
    [trackPath],
  );

  // Değişiklikler kısa bir beklemeden sonra kaydedilir; durmadan işaretlerken de en
  // geç AUTOSAVE_MAX_WAIT_MS içinde.
  useEffect(() => {
    if (!dirty || !trackPath) return;
    const since = dirtySince.current ?? performance.now();
    const wait = Math.min(
      AUTOSAVE_MS,
      Math.max(0, since + AUTOSAVE_MAX_WAIT_MS - performance.now()),
    );
    const timer = window.setTimeout(() => {
      const unsaved = pending.current;
      if (!unsaved || unsaved.path !== trackPath) return;
      setSaving(true);
      saveAnnotation(unsaved.path, unsaved.beats, unsaved.drops)
        .then((file) => {
          setSavedFile(file);
          setError(null);
          // Kayıt sürerken yeni işaret geldiyse kirli kalınır; o da kaydedilecek.
          if (changeCount.current === unsaved.change) {
            setDirty(false);
            dirtySince.current = null;
          } else {
            dirtySince.current = performance.now();
          }
        })
        .catch((e) => setError(errorMessage(e)))
        .finally(() => setSaving(false));
    }, wait);
    return () => window.clearTimeout(timer);
  }, [dirty, marks, trackPath]);

  const change = useCallback((next: (m: Marks) => Marks) => {
    setMarks(next);
    setDirty(true);
    changeCount.current += 1;
    dirtySince.current ??= performance.now();
    setSavedFile(null);
    setEvaluation(null); // eski sonuç artık bu işaretlere ait değil
  }, []);

  const markAt = useCallback(
    (kind: MarkKind, time: number) => {
      change((m) => addMark(m, kind, time));
      tapId.current += 1;
      setLastTap({ kind, id: tapId.current });
    },
    [change],
  );

  const mark = useCallback((kind: MarkKind) => markAt(kind, positionNow()), [markAt, positionNow]);
  const undo = useCallback(() => change(undoMark), [change]);
  const clear = useCallback(() => change(() => EMPTY_MARKS), [change]);
  const setRecording = useCallback((on: boolean) => setRecordingState(on), []);

  // İşaretleme sürerken klavye: oynatıcının kısayollarından önce yakalanır.
  useEffect(() => {
    if (!recording) return;
    const onKey = (event: KeyboardEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      if (target?.closest("input, textarea, select, [contenteditable]")) return;
      const key = event.code;
      if (key !== "Space" && key !== "KeyD" && key !== "Backspace" && key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      if (event.repeat) return;
      if (key === "Escape") {
        setRecordingState(false);
      } else if (key === "Backspace") {
        undo();
      } else if (playing) {
        const time = positionAtEvent(positionNow(), performance.now(), event.timeStamp);
        markAt(key === "Space" ? "beat" : "drop", time);
      }
    };
    window.addEventListener("keydown", onKey, { capture: true });
    return () => window.removeEventListener("keydown", onKey, { capture: true });
  }, [recording, playing, positionNow, markAt, undo]);

  const evaluate = useCallback(async () => {
    if (!trackPath) return;
    setEvaluating(true);
    try {
      const result = await evaluateAnnotation(trackPath);
      setEvaluation(result.evaluation);
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setEvaluating(false);
    }
  }, [trackPath]);

  return {
    marks,
    recording: recording && trackPath !== null,
    setRecording,
    lastTap,
    savedFile: dirty ? null : savedFile,
    saving,
    evaluation,
    evaluating,
    songMap,
    error,
    mark,
    undo,
    clear,
    evaluate,
  };
}
