import { useCallback, useEffect, useRef, useState } from "react";
import {
  errorMessage,
  evaluateAnnotation,
  getAnnotation,
  getSongMap,
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
/** Şarkı haritası (analiz) hazır olana kadar bu aralıkla sorulur; en fazla 2 dakika. */
const SONG_MAP_POLL_MS = 1000;
const SONG_MAP_MAX_TRIES = 120;

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
  const [songMap, setSongMap] = useState<SongMap | null>(null);
  const tapId = useRef(0);

  // Şarkı değişince o şarkının kayıtlı işaretleri yüklenir.
  if (loadedPath !== trackPath) {
    setLoadedPath(trackPath);
    setMarks(EMPTY_MARKS);
    setEvaluation(null);
    setSavedFile(null);
    setDirty(false);
    setError(null);
    setSongMap(null);
  }

  // Şarkı haritası analiz bitince gelir: hazır olana kadar ara ara sorulur.
  useEffect(() => {
    if (!trackPath) return;
    let cancelled = false;
    let tries = 0;
    let timer = 0;
    const ask = () => {
      getSongMap()
        .then((map) => {
          if (cancelled) return;
          if (map) setSongMap(map);
          else if (++tries < SONG_MAP_MAX_TRIES) timer = window.setTimeout(ask, SONG_MAP_POLL_MS);
        })
        .catch(() => {
          /* Analiz yoksa harita gösterilmez. */
        });
    };
    ask();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [trackPath]);
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

  // Değişiklikler kısa bir beklemeden sonra kaydedilir.
  useEffect(() => {
    if (!dirty || !trackPath) return;
    const timer = window.setTimeout(() => {
      setSaving(true);
      saveAnnotation(trackPath, marks.beats, marks.drops)
        .then((file) => {
          setSavedFile(file);
          setDirty(false);
          setError(null);
        })
        .catch((e) => setError(errorMessage(e)))
        .finally(() => setSaving(false));
    }, AUTOSAVE_MS);
    return () => window.clearTimeout(timer);
  }, [dirty, marks, trackPath]);

  const change = useCallback((next: (m: Marks) => Marks) => {
    setMarks(next);
    setDirty(true);
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
