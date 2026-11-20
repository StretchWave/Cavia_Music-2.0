import { useCallback, useEffect, useState } from 'react';

export type MotionSetting = 'full' | 'reduced' | 'off';

const STORAGE_KEY = 'cavia.motion';

function systemPrefersReduced(): boolean {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return false;
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

function readStoredMotion(): MotionSetting | null {
  try {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored === 'full' || stored === 'reduced' || stored === 'off') return stored;
  } catch {
    // Private mode or blocked storage: fall through to the system preference.
  }
  return null;
}

/**
 * Motion preference with an explicit in-app override.
 * Defaults to the OS `prefers-reduced-motion` setting so the app never
 * animates against the user's stated preference on first launch.
 */
export function useMotionSetting(): [MotionSetting, (next: MotionSetting) => void] {
  const [motion, setMotion] = useState<MotionSetting>(() => readStoredMotion() ?? (systemPrefersReduced() ? 'reduced' : 'full'));

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, motion);
    } catch {
      // Preference simply does not persist; the session still honours it.
    }
  }, [motion]);

  const update = useCallback((next: MotionSetting) => setMotion(next), []);

  return [motion, update];
}
