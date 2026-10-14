import { useEffect, useState } from "react";

/**
 * Klavyeyle (Tab) gelinen odak `selector`'a uyan bir öğenin içinde mi? Fareyle tıklanan
 * düğmede kalan odak sayılmaz: tıklayıp fareyi bırakınca sinema görünümü yine gelir.
 * (Tarayıcının `:focus-visible` kuralının aynısı; test ortamında da çalışsın diye elle.)
 */
export function useKeyboardFocusWithin(selector: string): boolean {
  const [inside, setInside] = useState(false);

  useEffect(() => {
    let tabbing = false;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Tab") tabbing = true;
    };
    const onPointerDown = () => {
      tabbing = false;
      setInside(false);
    };
    const onFocusIn = (event: FocusEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      setInside(tabbing && target?.closest(selector) != null);
    };
    const onFocusOut = (event: FocusEvent) => {
      if (!(event.relatedTarget instanceof Element)) setInside(false);
    };
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("focusin", onFocusIn);
    window.addEventListener("focusout", onFocusOut);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("focusin", onFocusIn);
      window.removeEventListener("focusout", onFocusOut);
    };
  }, [selector]);

  return inside;
}
