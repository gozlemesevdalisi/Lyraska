import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import {
  CloseIcon,
  FolderPlusIcon,
  MusicPlusIcon,
  PauseIcon,
  PlayIcon,
  RefreshIcon,
  SearchIcon,
} from "./icons";
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
  /** "Şarkı ekle" penceresinde gösterilecek uzantılar. */
  extensions: string[];
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
  extensions,
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
          title="Klasördeki bütün şarkıları ekleyin"
          aria-label="Klasör ekle"
        >
          <FolderPlusIcon />
          <span>Klasör ekle</span>
        </button>
        <button
          type="button"
          className="hw-button hw-button--small"
          onClick={() => void library.addFiles(extensions)}
          disabled={!available}
          title="Tek tek şarkı seçip ekleyin (birden fazla seçebilirsiniz)"
          aria-label="Şarkı ekle"
        >
          <MusicPlusIcon />
          <span>Şarkı ekle</span>
        </button>
        <button
          type="button"
          className="hw-button hw-button--small hw-button--icon"
          onClick={() => void library.rescan()}
          disabled={!available || noFolders || scan.scanning}
          title="Kütüphaneyi yeniden tara"
          aria-label="Kütüphaneyi yeniden tara"
        >
          <RefreshIcon />
        </button>
      </header>

      {status.folders.length > 0 && (
        <ul className="library__folders" aria-label="Eklenen klasörler ve şarkılar">
          {status.folders.map((folder) => (
            <li key={folder.id} className="chip" title={folder.path}>
              <span>{folderName(folder.path)}</span>
              <button
                type="button"
                onClick={() => void library.removeFolder(folder.id)}
                aria-label={`Kütüphaneden çıkar: ${folder.path}`}
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
          <p className="library__empty-title">Müziğinizi ekleyin</p>
          <p>
            <strong>Klasör ekle:</strong> klasördeki (alt klasörler dahil) bütün şarkılar eklenir.
            Bu pencerede şarkılar görünmez; klasörün içine girip "Klasör seç"e basmanız yeterli.
          </p>
          <p>
            <strong>Şarkı ekle:</strong> şarkıları (ör. mp3) tek tek ya da birkaçını birden seçip
            eklersiniz. Klasörleri bu pencereye sürükleyip de bırakabilirsiniz. Dosyalarınız
            yerinden oynamaz, internet gerekmez.
          </p>
          <div className="library__empty-actions">
            <button
              type="button"
              className="hw-button hw-button--primary"
              onClick={() => void library.addFolder()}
              disabled={!available}
            >
              <FolderPlusIcon />
              <span>Klasör ekle</span>
            </button>
            <button
              type="button"
              className="hw-button hw-button--primary"
              onClick={() => void library.addFiles(extensions)}
              disabled={!available}
            >
              <MusicPlusIcon />
              <span>Şarkı ekle</span>
            </button>
          </div>
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
