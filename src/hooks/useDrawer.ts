import { useCallback, useEffect, useState } from "react";

/** Sağ çekmecedeki paneller. */
export type Panel = "library" | "eq" | "marker" | "sync" | "settings";

export interface DrawerControls {
  open: boolean;
  panel: Panel;
  /** Paneli seçer (çekmece açıkken sekme değiştirmek için). */
  select: (panel: Panel) => void;
  /** Üst çubuktaki düğmeler: paneli açar; o panel zaten açıksa çekmeceyi kapatır. */
  toggle: (panel: Panel) => void;
  close: () => void;
}

export interface DrawerOptions {
  /** Panelden ayrılınca biten işler: işaretleme ve senkron ölçümü. */
  onLeave: (panel: Panel) => void;
  /** Esc başka bir işi bitirmek için kullanılıyorsa (ör. işaretleme) çekmeceyi kapatmaz. */
  escapeBusy: boolean;
}

/**
 * Sağdan açılan çekmece. Açılışta kütüphane açıktır: henüz bir şey çalmıyor,
 * kütüphane ilk iştir. Esc çekmeceyi kapatır (yazı yazılırken değil).
 */
export function useDrawer({ onLeave, escapeBusy }: DrawerOptions): DrawerControls {
  const [open, setOpen] = useState(true);
  const [panel, setPanel] = useState<Panel>("library");

  const select = useCallback(
    (next: Panel) => {
      if (next !== panel) onLeave(panel);
      setPanel(next);
    },
    [panel, onLeave],
  );
  const close = useCallback(() => {
    setOpen(false);
    onLeave(panel);
  }, [panel, onLeave]);
  const toggle = useCallback(
    (next: Panel) => {
      if (open && panel === next) {
        close();
      } else {
        select(next);
        setOpen(true);
      }
    },
    [open, panel, close, select],
  );

  useEffect(() => {
    if (!open || escapeBusy) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      const target = event.target instanceof Element ? event.target : null;
      if (target?.closest("input, textarea, select, [contenteditable]")) return;
      close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, escapeBusy, close]);

  return { open, panel, select, toggle, close };
}
