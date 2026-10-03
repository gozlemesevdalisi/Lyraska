import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { CloseIcon, FolderPlusIcon, PauseIcon, PlayIcon, RefreshIcon, SearchIcon } from "./icons";
import type { LibraryTrack } from "../lib/backend";
import { formatTime } from "../lib/format";
import { scrollToRow, visibleRange } from "../lib/virtual";
import type { LibraryControls } from "../hooks/useLibrary";

/** Satır yüksekliği (CSS'teki `--row-height` ile aynı olmalı). */
export const ROW_HEIGHT = 34;

const NUMBER = new Intl.NumberFormat("tr-TR");

export interface LibraryPanelProps {
  library: LibraryControls;
  available: boolean;
  /** Çalan şarkının yolu (listede vurgulanır). */
  currentPath: string | null;
  playing: boolean;
  onPlay: (track: LibraryTrack, list: LibraryTrack[]) => void;
}

/** Klasör yolunun son parçası ("D:\Müzik\Rock" → "Rock"). */
export function folderName(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

/** Kütüphane: arama, klasörler, tarama durumu ve şarkı listesi. */
export function LibraryPanel({
  library,
  available,
  currentPath,
  playing,
  onPlay,
}: LibraryPanelProps) {
  const { status, tracks, query } = library;
  const [selected, setSelected] = useState(-1);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewport, setViewport] = useState(400);
  const listRef = useRef<HTMLDivElement>(null);

  // Liste yüksekliğini izle (pencere boyutu değişince).
  useEffect(() => {
    const element = listRef.current;
    if (!element) return;
    const update = () => setViewport(element.clientHeight || 400);
    update();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const select = (index: number) => {
    const clamped = Math.max(0, Math.min(tracks.length - 1, index));
    setSelected(clamped);
    const element = listRef.current;
    if (!element) return;
    const target = scrollToRow(clamped, element.scrollTop, element.clientHeight, ROW_HEIGHT);
    if (target !== null) element.scrollTop = target;
  };

  const onListKey = (event: KeyboardEvent<HTMLDivElement>) => {
    if (tracks.length === 0) return;
    const page = Math.max(1, Math.floor(viewport / ROW_HEIGHT) - 1);
    const moves: Record<string, number> = {
      ArrowDown: 1,
      ArrowUp: -1,
      PageDown: page,
      PageUp: -page,
    };
    if (event.key in moves) {
      event.preventDefault();
      select((selected < 0 ? -1 : selected) + moves[event.key]!);
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      select(event.key === "Home" ? 0 : tracks.length - 1);
    } else if (event.key === "Enter" && selected >= 0) {
      event.preventDefault();
      onPlay(tracks[selected]!, tracks);
    }
  };

  const range = visibleRange(scrollTop, viewport, ROW_HEIGHT, tracks.length);
  const scan = status.scan;
  const noFolders = status.folders.length === 0;

  return (
    <section className="library" aria-label="Kütüphane">
      <header className="library__header">
        <h2 className="library__title">
          Kütüphane
          <span className="library__count">{NUMBER.format(status.trackCount)} şarkı</span>
        </h2>
        <label className="library__search">
          <SearchIcon />
          <input
            type="search"
            value={query}
            onChange={(e) => library.setQuery(e.target.value)}
            placeholder="Ara: şarkı, sanatçı, albüm"
            aria-label="Kütüphanede ara"
            disabled={!available || noFolders}
            spellCheck={false}
          />
        </label>
        <button
          type="button"
          className="hw-button hw-button--small"
          onClick={() => void library.addFolder()}
          disabled={!available}
        >
          <FolderPlusIcon />
          <span>Klasör ekle</span>
        </button>
        <button
          type="button"
          className="hw-button hw-button--small hw-button--icon"
          onClick={() => void library.rescan()}
          disabled={!available || noFolders || scan.scanning}
          title="Klasörleri yeniden tara"
          aria-label="Klasörleri yeniden tara"
        >
          <RefreshIcon />
        </button>
      </header>

      {status.folders.length > 0 && (
        <ul className="library__folders" aria-label="Klasörler">
          {status.folders.map((folder) => (
            <li key={folder.id} className="chip" title={folder.path}>
              <span>{folderName(folder.path)}</span>
              <button
                type="button"
                onClick={() => void library.removeFolder(folder.id)}
                aria-label={`Klasörü kütüphaneden çıkar: ${folder.path}`}
                title="Kütüphaneden çıkar (dosyalar silinmez)"
              >
                <CloseIcon />
              </button>
            </li>
          ))}
        </ul>
      )}

      {(scan.scanning || status.problems.length > 0 || library.error) && (
        <div className="library__notice" role="status">
          {scan.scanning && (
            <span>
              Taranıyor: {NUMBER.format(scan.processed)} / {NUMBER.format(scan.found)} dosya
              {scan.current ? ` · ${folderName(scan.current)}` : ""}
            </span>
          )}
          {[...status.problems, ...(library.error ? [library.error] : [])].map((problem) => (
            <span key={problem} className="library__problem">
              {problem}
            </span>
          ))}
        </div>
      )}

      {noFolders ? (
        <div className="library__empty">
          <p className="library__empty-title">Müzik klasörünüzü ekleyin</p>
          <p>
            Lyraska klasördeki (alt klasörler dahil) bütün şarkıları bulur; sanatçı, albüm ve şarkı
            adlarıyla listeler. Dosyalarınız yerinden oynamaz, internet gerekmez.
          </p>
          <p>
            Klasörü bu pencereye sürükleyip bırakabilirsiniz. "Klasör ekle" penceresinde şarkılar
            görünmez; klasörün içine girip "Klasör seç" düğmesine basmanız yeterli.
          </p>
          <button
            type="button"
            className="hw-button hw-button--primary"
            onClick={() => void library.addFolder()}
            disabled={!available}
          >
            <FolderPlusIcon />
            <span>Klasör ekle</span>
          </button>
        </div>
      ) : (
        <div className="library__table" role="grid" aria-rowcount={tracks.length}>
          <div className="library__row library__row--head" role="row">
            <span role="columnheader">#</span>
            <span role="columnheader">Başlık</span>
            <span role="columnheader">Sanatçı</span>
            <span role="columnheader" className="library__album">
              Albüm
            </span>
            <span role="columnheader" className="library__time">
              Süre
            </span>
          </div>
          <div
            ref={listRef}
            className="library__list"
            tabIndex={0}
            onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
            onKeyDown={onListKey}
            aria-label="Şarkılar"
          >
            {tracks.length === 0 ? (
              <p className="library__none">
                {query ? `“${query}” için sonuç yok.` : scan.scanning ? "Taranıyor…" : "Şarkı yok."}
              </p>
            ) : (
              <div style={{ height: tracks.length * ROW_HEIGHT, position: "relative" }}>
                {tracks.slice(range.start, range.end).map((track, offset) => {
                  const index = range.start + offset;
                  const isCurrent = track.path === currentPath;
                  return (
                    <div
                      key={track.id}
                      role="row"
                      aria-rowindex={index + 1}
                      aria-selected={index === selected}
                      className={`library__row${isCurrent ? " is-current" : ""}${index === selected ? " is-selected" : ""}`}
                      style={{ transform: `translateY(${index * ROW_HEIGHT}px)` }}
                      onClick={() => setSelected(index)}
                      onDoubleClick={() => onPlay(track, tracks)}
                      title={track.path}
                    >
                      <span className="library__no">
                        {isCurrent ? (
                          playing ? (
                            <PlayIcon />
                          ) : (
                            <PauseIcon />
                          )
                        ) : (
                          (track.trackNumber ?? "")
                        )}
                      </span>
                      <span className="library__cell">{track.title}</span>
                      <span className="library__cell library__muted">{track.artist ?? "—"}</span>
                      <span className="library__cell library__muted library__album">
                        {track.album ?? "—"}
                      </span>
                      <span className="library__time">
                        {track.durationSecs ? formatTime(track.durationSecs) : ""}
                      </span>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
