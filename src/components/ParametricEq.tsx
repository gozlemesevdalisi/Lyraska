import {
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
} from "react";
import type { ParametricControls } from "../hooks/useParametric";
import type { FilterKind, ParametricBand } from "../lib/backend";
import {
  PEQ_KINDS,
  dbToY,
  formatDb,
  formatFrequency,
  formatQ,
  hzToX,
  kindHasGain,
  kindLabel,
  newBand,
  nodeDb,
  parseDecimal,
  parseFrequency,
  peqPath,
  plotRangeDb,
  qToOctaves,
  roundGain,
  roundHz,
  roundQ,
  stepHz,
  stepQ,
  xToHz,
  yToDb,
} from "../lib/parametric";

const PLOT_WIDTH = 1000;
const PLOT_HEIGHT = 320;
const GRID_HZ = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
const AXIS_HZ: [number, string][] = [
  [20, "20"],
  [100, "100"],
  [1000, "1k"],
  [10000, "10k"],
  [20000, "20k"],
];

export interface ParametricEqProps {
  parametric: ParametricControls;
  /** Bit-perfect çalıyor: ses işlenmiyor. */
  bypassed?: boolean;
}

/** Sayı kutusu: yazarken taslak tutulur; Enter ya da odak çıkınca uygulanır, Esc geri alır. */
function NumberField({
  label,
  value,
  format,
  parse,
  onCommit,
  disabled = false,
  unit,
}: {
  label: string;
  value: number;
  format: (value: number) => string;
  parse: (text: string) => number | null;
  onCommit: (value: number) => void;
  disabled?: boolean;
  unit?: string;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const commit = () => {
    if (draft === null) return;
    const parsed = parse(draft);
    setDraft(null);
    if (parsed !== null) onCommit(parsed);
  };
  return (
    <label className={`peq-field${disabled ? " is-disabled" : ""}`}>
      <span className="peq-field__label">{label}</span>
      <span className="peq-field__box">
        <input
          type="text"
          inputMode="decimal"
          aria-label={label}
          value={draft ?? format(value)}
          disabled={disabled}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={commit}
          onKeyDown={(event) => {
            // Yazarken kısayollar (boşluk: çal/duraklat, oklar: sar) çalışmasın.
            event.stopPropagation();
            if (event.key === "Enter") commit();
            if (event.key === "Escape") setDraft(null);
          }}
        />
        {unit && <span className="peq-field__unit">{unit}</span>}
      </span>
    </label>
  );
}

/** Seçili bandın düzenleyicisi: tür, frekans, kazanç, Q, aç/kapat, sil. */
function BandEditor({
  band,
  index,
  onChange,
  onRemove,
}: {
  band: ParametricBand;
  index: number;
  onChange: (band: ParametricBand) => void;
  onRemove: () => void;
}) {
  const hasGain = kindHasGain(band.kind);
  return (
    <div className="peq-editor" role="group" aria-label={`Bant ${index + 1}`}>
      <span className="peq-editor__number" aria-hidden>
        {index + 1}
      </span>
      <label className="peq-field">
        <span className="peq-field__label">Tür</span>
        <select
          className="peq-field__select"
          aria-label="Tür"
          value={band.kind}
          onChange={(event) => onChange({ ...band, kind: event.target.value as FilterKind })}
          onKeyDown={(event) => event.stopPropagation()}
        >
          {PEQ_KINDS.map((k) => (
            <option key={k.kind} value={k.kind}>
              {k.short} · {k.name}
            </option>
          ))}
        </select>
      </label>
      <NumberField
        label="Frekans"
        value={band.freqHz}
        format={(hz) =>
          hz >= 1000
            ? `${(hz / 1000).toLocaleString("tr-TR", { maximumFractionDigits: 3 })}k`
            : hz.toLocaleString("tr-TR", { maximumFractionDigits: 1 })
        }
        parse={parseFrequency}
        unit="Hz"
        onCommit={(hz) => onChange({ ...band, freqHz: roundHz(hz) })}
      />
      <NumberField
        label="Kazanç"
        value={band.gainDb}
        format={(db) => formatDb(db)}
        parse={parseDecimal}
        unit="dB"
        disabled={!hasGain}
        onCommit={(db) => onChange({ ...band, gainDb: roundGain(db) })}
      />
      <NumberField
        label="Q"
        value={band.q}
        format={formatQ}
        parse={parseDecimal}
        onCommit={(q) => onChange({ ...band, q: roundQ(q) })}
      />
      <span className="peq-editor__bw" title="Q'nun karşılığı bant genişliği">
        {kindHasGain(band.kind) || band.kind === "bandPass" || band.kind === "notch"
          ? `${qToOctaves(band.q).toLocaleString("tr-TR", { maximumFractionDigits: 2 })} oktav`
          : ""}
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={band.enabled}
        aria-label={`Bant ${index + 1} açık`}
        className={`switch${band.enabled ? " is-on" : ""}`}
        onClick={() => onChange({ ...band, enabled: !band.enabled })}
        title="Bandı aç / kapat (ayarı kaybolmaz)"
      >
        <span className="switch__knob" aria-hidden />
        <span>{band.enabled ? "Açık" : "Kapalı"}</span>
      </button>
      <button type="button" className="chip chip--button" onClick={onRemove}>
        Sil
      </button>
    </div>
  );
}

/**
 * Elle parametrik ekolayzer: grafikte sürüklenen bantlar (yatay frekans, dikey kazanç; tekerlek
 * Q), seçili bandın sayısal düzenleyicisi, eşit ses yüksekliğinde karşılaştırma (A/B), Equalizer
 * APO dosyası yükleme/kaydetme. Çizilen eğri, çekirdeğin hesapladığı gerçek sayısal eğridir.
 */
export function ParametricEq({ parametric, bypassed = false }: ParametricEqProps) {
  const { settings, state, comparing, update, error, busy } = parametric;
  const [selected, setSelected] = useState(0);
  const plot = useRef<HTMLDivElement | null>(null);
  const drag = useRef<{
    index: number;
    startX: number;
    startY: number;
    hz: number;
    db: number;
  } | null>(null);
  const bands = settings.bands;
  const current = Math.min(selected, bands.length - 1);
  const range = plotRangeDb([
    state.curveDb,
    bands.map((b) => (kindHasGain(b.kind) ? b.gainDb : 0)),
  ]);
  const maxBands = state.maxBands;

  const setBand = (index: number, change: (band: ParametricBand) => ParametricBand) =>
    update((s) => ({ ...s, bands: s.bands.map((b, i) => (i === index ? change(b) : b)) }));
  const removeBand = (index: number) => {
    update((s) => ({ ...s, bands: s.bands.filter((_, i) => i !== index) }));
    setSelected((i) => Math.max(0, Math.min(i, bands.length - 2)));
  };
  const addBand = (hz: number, db: number) => {
    if (bands.length >= maxBands) return;
    update((s) => ({ ...s, bands: [...s.bands, newBand(hz, db)] }));
    setSelected(bands.length);
  };

  /** İşaretçinin grafikteki konumu (0..1, 0..1). */
  const position = (clientX: number, clientY: number) => {
    const rect = plot.current?.getBoundingClientRect();
    if (!rect || rect.width <= 0 || rect.height <= 0) return null;
    return { x: (clientX - rect.left) / rect.width, y: (clientY - rect.top) / rect.height };
  };

  // Tekerlek seçili bandın Q'sunu değiştirir (sayfa kaymasın diye pasif olmayan dinleyici).
  const selectedRef = useRef(current);
  const updateRef = useRef(update);
  useEffect(() => {
    selectedRef.current = current;
    updateRef.current = update;
  });
  useEffect(() => {
    const element = plot.current;
    if (!element) return;
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      const steps = (event.deltaY < 0 ? 1 : -1) * (event.shiftKey ? 0.25 : 1);
      const index = selectedRef.current;
      updateRef.current((s) => ({
        ...s,
        bands: s.bands.map((b, i) => (i === index ? { ...b, q: stepQ(b.q, steps) } : b)),
      }));
    };
    element.addEventListener("wheel", onWheel, { passive: false });
    return () => element.removeEventListener("wheel", onWheel);
  }, []);

  const onNodePointerDown = (event: PointerEvent<HTMLButtonElement>, index: number) => {
    if (event.button !== 0) return;
    const band = bands[index];
    if (!band) return;
    event.stopPropagation();
    setSelected(index);
    event.currentTarget.setPointerCapture?.(event.pointerId);
    event.currentTarget.focus();
    drag.current = {
      index,
      startX: event.clientX,
      startY: event.clientY,
      hz: band.freqHz,
      db: band.gainDb,
    };
  };

  const onNodePointerMove = (event: PointerEvent<HTMLButtonElement>) => {
    const start = drag.current;
    if (!start) return;
    const rect = plot.current?.getBoundingClientRect();
    if (!rect || rect.width <= 0 || rect.height <= 0) return;
    // Shift: ince ayar (hareketin beşte biri).
    const fine = event.shiftKey ? 0.2 : 1;
    const dx = ((event.clientX - start.startX) / rect.width) * fine;
    const dy = ((event.clientY - start.startY) / rect.height) * fine;
    const hz = xToHz(hzToX(start.hz) + dx);
    const db = yToDb(dbToY(start.db, range) + dy, range);
    setBand(start.index, (b) => ({
      ...b,
      freqHz: roundHz(hz),
      gainDb: kindHasGain(b.kind) ? roundGain(db) : b.gainDb,
    }));
  };

  const endDrag = () => {
    drag.current = null;
  };

  const onNodeKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const fine = event.shiftKey;
    const band = bands[index];
    if (!band) return;
    let handled = true;
    switch (event.key) {
      case "ArrowLeft":
        setBand(index, (b) => ({ ...b, freqHz: stepHz(b.freqHz, fine ? -1 / 48 : -1 / 12) }));
        break;
      case "ArrowRight":
        setBand(index, (b) => ({ ...b, freqHz: stepHz(b.freqHz, fine ? 1 / 48 : 1 / 12) }));
        break;
      case "ArrowUp":
        if (kindHasGain(band.kind))
          setBand(index, (b) => ({ ...b, gainDb: roundGain(b.gainDb + (fine ? 0.1 : 0.5)) }));
        break;
      case "ArrowDown":
        if (kindHasGain(band.kind))
          setBand(index, (b) => ({ ...b, gainDb: roundGain(b.gainDb - (fine ? 0.1 : 0.5)) }));
        break;
      case "PageUp":
        setBand(index, (b) => ({ ...b, q: stepQ(b.q, fine ? 0.25 : 1) }));
        break;
      case "PageDown":
        setBand(index, (b) => ({ ...b, q: stepQ(b.q, fine ? -0.25 : -1) }));
        break;
      case "Delete":
      case "Backspace":
        removeBand(index);
        break;
      case "0":
        if (kindHasGain(band.kind)) setBand(index, (b) => ({ ...b, gainDb: 0 }));
        break;
      default:
        handled = false;
    }
    if (handled) {
      // Oklar şarkıyı sarmasın, boşluk çalmayı durdurmasın.
      event.preventDefault();
      event.stopPropagation();
    }
  };

  const onPlotDoubleClick = (event: MouseEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget && !(event.target instanceof SVGElement)) return;
    const at = position(event.clientX, event.clientY);
    if (at) addBand(xToHz(at.x), yToDb(at.y, range));
  };

  const totalPath = peqPath(state.curveHz, state.curveDb, PLOT_WIDTH, PLOT_HEIGHT, range);
  const zeroY = dbToY(0, range) * PLOT_HEIGHT;
  const area = totalPath ? `${totalPath}L${PLOT_WIDTH},${zeroY}L0,${zeroY}Z` : "";
  const gridDb: number[] = [];
  for (let db = -range; db <= range; db += 6) gridDb.push(db);
  const editing = bands[current];
  const enabled = settings.enabled;

  return (
    <section className="peq" aria-label="Parametrik ekolayzer">
      <header className="library__header eq__header">
        <h2 className="library__title">Parametrik ekolayzer</h2>
        <button
          type="button"
          role="switch"
          aria-checked={enabled}
          aria-label="Parametrik ekolayzer"
          className={`switch${enabled ? " is-on" : ""}`}
          onClick={() => update((s) => ({ ...s, enabled: !s.enabled }))}
          title="Parametrik ekolayzeri aç / kapat (ayarlar kaybolmaz)"
        >
          <span className="switch__knob" aria-hidden />
          <span>{enabled ? "Açık" : "Kapalı"}</span>
        </button>
        <button
          type="button"
          className={`chip chip--button peq__compare${comparing ? " is-selected" : ""}`}
          aria-pressed={comparing}
          disabled={!parametric.active}
          onClick={() => parametric.setCompare(!comparing)}
          title="Ekolayzersiz dinle: ses, ekolayzerli hâliyle aynı yükseklikte çalar; yüksek olan daha iyi gelir yanılgısı olmaz."
        >
          {comparing ? "Karşılaştırma: ekolayzersiz" : "A/B karşılaştır"}
        </button>
        <span
          className="eq__preamp"
          title="Yükseltilen frekanslar sesi bozmasın diye ses, şarkının tepesindeki boşluğun yetmediği kadar kısılır."
        >
          En büyük yükseltme {formatDb(state.boostDb)} dB · ses yüksekliği{" "}
          {formatDb(state.loudnessDb)} dB
        </span>
      </header>

      {bypassed && (
        <p className="library__notice library__problem">
          Bit-perfect açık: ses hiç işlenmiyor, parametrik ekolayzer şimdilik devre dışı.
        </p>
      )}

      <div className="peq__design" role="group" aria-label="Süzgeç tasarımı">
        <span className="peq__design-label">Tasarım</span>
        <button
          type="button"
          className={`chip chip--button${settings.design === "matched" ? " is-selected" : ""}`}
          aria-pressed={settings.design === "matched"}
          onClick={() => update((s) => ({ ...s, design: "matched" }))}
          title="Süzgeçler analog tasarımlarına eşlenir: çizilen eğri 20 kHz'e kadar duyulan eğridir (önerilen)."
        >
          Analog eğriye sadık
        </button>
        <button
          type="button"
          className={`chip chip--button${settings.design === "rbj" ? " is-selected" : ""}`}
          aria-pressed={settings.design === "rbj"}
          onClick={() => update((s) => ({ ...s, design: "rbj" }))}
          title="Equalizer APO ve AutoEq'in formülleri: oralarda hazırlanmış bir ayar bire bir aynı duyulur."
        >
          Equalizer APO ile aynı
        </button>
      </div>

      {error && <p className="library__notice library__problem">{error}</p>}

      <div className={`peq__board${enabled ? "" : " is-off"}${comparing ? " is-comparing" : ""}`}>
        <div className="eq__scale" aria-hidden>
          {gridDb
            .filter((db) => db % 12 === 0)
            .map((db) => (
              <span key={db} style={{ top: `${dbToY(db, range) * 100}%` }}>
                {formatDb(db)}
              </span>
            ))}
        </div>
        <div
          ref={plot}
          className="peq__plot"
          onDoubleClick={onPlotDoubleClick}
          title="Bandı sürükleyin (Shift: ince ayar); tekerlek: Q; boş yere çift tıklayın: bant ekle"
        >
          <svg
            className="eq__curve"
            viewBox={`0 0 ${PLOT_WIDTH} ${PLOT_HEIGHT}`}
            preserveAspectRatio="none"
            aria-hidden
          >
            {GRID_HZ.map((hz) => (
              <line
                key={hz}
                className="eq__grid"
                x1={hzToX(hz) * PLOT_WIDTH}
                x2={hzToX(hz) * PLOT_WIDTH}
                y1={0}
                y2={PLOT_HEIGHT}
              />
            ))}
            {gridDb.map((db) => (
              <line
                key={db}
                className={db === 0 ? "eq__grid eq__grid--zero" : "eq__grid"}
                x1={0}
                x2={PLOT_WIDTH}
                y1={dbToY(db, range) * PLOT_HEIGHT}
                y2={dbToY(db, range) * PLOT_HEIGHT}
              />
            ))}
            {state.bandCurvesDb.map((curve, i) => (
              <path
                key={i}
                className={`peq__band-line${i === current ? " is-selected" : ""}${
                  bands[i]?.enabled ? "" : " is-disabled"
                }`}
                d={peqPath(state.curveHz, curve, PLOT_WIDTH, PLOT_HEIGHT, range)}
              />
            ))}
            <path className="eq__area" d={area} />
            <path className="eq__line" d={totalPath} />
          </svg>
          {bands.map((band, i) => (
            <button
              key={i}
              type="button"
              className={`peq-node${i === current ? " is-selected" : ""}${
                band.enabled ? "" : " is-disabled"
              }`}
              style={{
                left: `${hzToX(band.freqHz) * 100}%`,
                top: `${dbToY(nodeDb(band), range) * 100}%`,
              }}
              aria-label={`Bant ${i + 1}: ${kindLabel(band.kind)}, ${formatFrequency(band.freqHz)}${
                kindHasGain(band.kind) ? `, ${formatDb(band.gainDb)} dB` : ""
              }, Q ${formatQ(band.q)}${band.enabled ? "" : ", kapalı"}`}
              aria-describedby="peq-keys"
              onPointerDown={(event) => onNodePointerDown(event, i)}
              onPointerMove={onNodePointerMove}
              onPointerUp={endDrag}
              onPointerCancel={endDrag}
              onFocus={() => setSelected(i)}
              onKeyDown={(event) => onNodeKeyDown(event, i)}
              onDoubleClick={(event) => event.stopPropagation()}
            >
              {i + 1}
            </button>
          ))}
        </div>
        <div className="eq__axis" aria-hidden>
          {AXIS_HZ.map(([hz, label]) => (
            <span key={hz} style={{ left: `${hzToX(hz) * 100}%` }}>
              {label}
            </span>
          ))}
        </div>
      </div>
      <p id="peq-keys" className="peq__keys">
        Bant seçiliyken: ←/→ frekans, ↑/↓ kazanç, PageUp/PageDown Q (Shift ile ince), Delete siler,
        0 kazancı sıfırlar.
      </p>

      {editing && (
        <BandEditor
          key={current}
          band={editing}
          index={current}
          onChange={(band) => setBand(current, () => band)}
          onRemove={() => removeBand(current)}
        />
      )}

      <div className="peq__bands" role="group" aria-label="Bantlar">
        {bands.map((band, i) => (
          <button
            key={i}
            type="button"
            className={`chip chip--button${i === current ? " is-selected" : ""}${
              band.enabled ? "" : " is-dimmed"
            }`}
            aria-pressed={i === current}
            onClick={() => setSelected(i)}
          >
            {i + 1} · {PEQ_KINDS.find((k) => k.kind === band.kind)?.short}{" "}
            {formatFrequency(band.freqHz)}
            {kindHasGain(band.kind) ? ` ${formatDb(band.gainDb)}` : ""}
          </button>
        ))}
        <button
          type="button"
          className="chip chip--button"
          disabled={bands.length >= maxBands}
          onClick={() => addBand(1000, 0)}
        >
          + Bant ekle
        </button>
      </div>

      <div className="peq__actions">
        <button
          type="button"
          className="chip chip--button"
          disabled={busy}
          onClick={() => void parametric.importFile()}
          title="AutoEq, REW ya da Equalizer APO'nun parametrik dosyası (Equalizer APO biçimi)"
        >
          Dosyadan yükle
        </button>
        <button
          type="button"
          className="chip chip--button"
          disabled={busy}
          onClick={() => void parametric.exportFile()}
          title="Equalizer APO biçiminde kaydeder (Equalizer APO, Peace, Wavelet okur)"
        >
          Dosyaya kaydet
        </button>
        <button type="button" className="chip chip--button" onClick={parametric.reset}>
          Sıfırla
        </button>
      </div>
    </section>
  );
}
