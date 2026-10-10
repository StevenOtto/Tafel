/**
 * Mount/enter/exit state for overlays that must animate both ways. React's
 * conditional render can't transition on unmount, so we keep the node mounted
 * through the exit: `mounted` gates rendering, `shown` drives the enter/exit
 * transition, and the element is removed only after `exitMs` has passed.
 */

import { useCallback, useEffect, useRef, useState } from "react";

export interface Disclosure {
  readonly mounted: boolean;
  readonly shown: boolean;
  readonly open: () => void;
  readonly close: () => void;
}

export const useDisclosure = (exitMs: number): Disclosure => {
  const [mounted, setMounted] = useState(false);
  const [shown, setShown] = useState(false);
  const timer = useRef<number | null>(null);

  const clear = () => {
    if (timer.current !== null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
  };

  const open = useCallback(() => {
    clear();
    setMounted(true);
    // Two frames: let the browser paint the closed state before flipping to
    // open, so the enter transition actually runs.
    requestAnimationFrame(() => requestAnimationFrame(() => setShown(true)));
  }, []);

  const close = useCallback(() => {
    setShown(false);
    clear();
    timer.current = window.setTimeout(() => setMounted(false), exitMs);
  }, [exitMs]);

  useEffect(() => () => clear(), []);

  return { mounted, shown, open, close };
};
