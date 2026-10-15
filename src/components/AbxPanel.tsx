import type { AbxControls } from "../hooks/useAbx";
import type { AbxChoice } from "../lib/backend";
import {
  CLIP_CHOICES,
  TRIAL_CHOICES,
  baseName,
  neededCorrect,
  setupSummary,
  verdict,
} from "../lib/abx";

export interface AbxPanelProps {
  abx: AbxControls;
  /** Ses çalınabilir mi (tarayıcı önizlemesinde hayır). */
  available: boolean;
}

const CHOICES: [AbxChoice, string][] = [
  ["a", "A"],
  ["b", "B"],
  ["x", "X"],
];

function Setup({ abx, available }: AbxPanelProps) {
  const ready = abx.a !== null && abx.b !== null && !abx.busy && available;
  return (
    <>
      <div className="marker__help">
        <p>
          Aynı şarkının iki sürümünü seçin (ör. <strong>FLAC</strong> ve <strong>MP3</strong>).
          Program ikisinin aynı bölümünü hizalar, ses yüksekliklerini eşitler ve sesi hiç işlemeden
          çalar: ekolayzer ve eşitleme bu testte kapalıdır.
        </p>
        <p>
          Her denemede <strong>X</strong> gizlice A ya da B'dir. A, B ve X'i istediğiniz kadar
          dinleyip X'in hangisi olduğunu söylersiniz. Sonuç en sonda gösterilir.
        </p>
      </div>

      <div className="abx__files">
        {(
          [
            ["A", abx.a, abx.pickA],
            ["B", abx.b, abx.pickB],
          ] as const
        ).map(([label, path, pick]) => (
          <div key={label} className="abx__file">
            <span className="abx__letter" aria-hidden>
              {label}
            </span>
            <span className="abx__name" title={path ?? undefined}>
              {path ? baseName(path) : "Dosya seçilmedi"}
            </span>
            <button
              type="button"
              className="chip chip--button"
              aria-label={`${label} dosyasını seç`}
              onClick={() => void pick()}
            >
              Seç
            </button>
          </div>
        ))}
      </div>

      <div className="sync__setting abx__settings">
        <label htmlFor="abx-start">Başlangıç</label>
        <input
          id="abx-start"
          type="number"
          min={0}
          step={1}
          value={abx.startSecs}
          onChange={(e) => abx.setStartSecs(Math.max(0, Number(e.currentTarget.value) || 0))}
          aria-describedby="abx-start-unit"
        />
        <span id="abx-start-unit">saniye</span>
        <label htmlFor="abx-clip">Süre</label>
        <select
          id="abx-clip"
          value={abx.clipSecs}
          onChange={(e) => abx.setClipSecs(Number(e.currentTarget.value))}
        >
          {CLIP_CHOICES.map((s) => (
            <option key={s} value={s}>
              {s} sn
            </option>
          ))}
        </select>
        <label htmlFor="abx-trials">Deneme</label>
        <select
          id="abx-trials"
          value={abx.trials}
          onChange={(e) => abx.setTrials(Number(e.currentTarget.value))}
        >
          {TRIAL_CHOICES.map((n) => (
            <option key={n} value={n}>
              {n}
            </option>
          ))}
        </select>
      </div>
      <p className="sync__caption">
        {abx.trials} denemede en az {neededCorrect(abx.trials)} doğru, farkı duyduğunuzu gösterir
        (şansla çıkma olasılığı %5'in altında).
      </p>

      <div className="marker__actions">
        <button
          type="button"
          className="hw-button hw-button--primary"
          disabled={!ready}
          onClick={() => void abx.start()}
        >
          {abx.busy ? "Hazırlanıyor…" : "Testi başlat"}
        </button>
      </div>
    </>
  );
}

function Running({ abx }: { abx: AbxControls }) {
  const { status } = abx;
  const progress = status.clipSecs > 0 ? Math.min(1, status.positionSecs / status.clipSecs) : 0;
  return (
    <>
      <p className="sync__caption">
        A: {status.aName} · B: {status.bName}
        <br />
        {setupSummary(status)}
      </p>
      <div
        className="abx__progress"
        role="progressbar"
        aria-label="Bölümdeki konum"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(progress * 100)}
      >
        <span style={{ width: `${(progress * 100).toFixed(1)}%` }} />
      </div>

      <div className="abx__choices" role="group" aria-label="Dinle">
        {CHOICES.map(([choice, label]) => (
          <button
            key={choice}
            type="button"
            className={`hw-button abx__choice${status.playing === choice ? " is-selected" : ""}`}
            aria-pressed={status.playing === choice}
            onClick={() => void abx.select(choice)}
          >
            {label}
          </button>
        ))}
        <button
          type="button"
          className="hw-button abx__choice abx__pause"
          disabled={status.playing === null}
          onClick={() => void abx.pause()}
        >
          Duraklat
        </button>
      </div>

      <div className="abx__answer" aria-live="polite">
        <p className="abx__trial">
          Deneme {Math.min(status.answered + 1, status.total)} / {status.total}: X hangisi?
        </p>
        <div className="marker__actions">
          <button
            type="button"
            className="hw-button hw-button--primary"
            onClick={() => void abx.answer("a")}
          >
            X = A
          </button>
          <button
            type="button"
            className="hw-button hw-button--primary"
            onClick={() => void abx.answer("b")}
          >
            X = B
          </button>
          <button type="button" className="chip chip--button" onClick={() => void abx.stop()}>
            Testi bitir
          </button>
        </div>
      </div>
    </>
  );
}

function Result({ abx }: { abx: AbxControls }) {
  const result = abx.status.result;
  if (!result) return null;
  const v = verdict(result);
  return (
    <>
      <div className={`abx__result${v.heard ? " is-heard" : ""}`} role="status">
        <h3>{v.title}</h3>
        <p>{v.detail}</p>
      </div>
      <p className="sync__caption">
        Ölçüt: sonucun yalnızca şansla çıkma olasılığı %5'in altındaysa fark duyuluyor sayılır.
        Başka bir bölümle ya da daha çok denemeyle tekrarlamak sonucu güçlendirir.
      </p>
      <div className="marker__actions">
        <button
          type="button"
          className="hw-button hw-button--primary"
          disabled={abx.busy}
          onClick={() => void abx.start()}
        >
          {abx.busy ? "Hazırlanıyor…" : "Aynı dosyalarla yeniden"}
        </button>
        <button type="button" className="chip chip--button" onClick={() => void abx.stop()}>
          Kapat
        </button>
      </div>
    </>
  );
}

/** Kör dinleme testi (ABX): iki sürümü hangisinin çaldığını bilmeden karşılaştırma. */
export function AbxPanel({ abx, available }: AbxPanelProps) {
  const { status } = abx;
  return (
    <section className="marker abx" aria-label="Kör test">
      <header className="library__header marker__header">
        <h2 className="library__title">Kör test</h2>
        <span className="marker__track">
          İki sürümü, hangisinin çaldığını bilmeden karşılaştırın.
        </span>
      </header>
      <div className="marker__body">
        {!status.active ? (
          <Setup abx={abx} available={available} />
        ) : status.result ? (
          <Result abx={abx} />
        ) : (
          <Running abx={abx} />
        )}
        {abx.error && <p className="library__notice library__problem">{abx.error}</p>}
      </div>
    </section>
  );
}
