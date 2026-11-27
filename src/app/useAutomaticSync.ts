import { useCallback, useEffect, useRef } from 'react';
import { isNativeApp } from '../lib/platform';
import { getSyncStatus, syncNow } from '../lib/sync';

/** Coordinates debounced sync attempts with bounded retry backoff. */
export function useAutomaticSync() {
  const timerRef = useRef<number | null>(null);
  const runningRef = useRef(false);
  const pendingRef = useRef(false);
  const retryDelayRef = useRef(2_000);

  const scheduleAutomaticSync = useCallback((delay = 1_200) => {
    if (!isNativeApp()) return;
    if (timerRef.current !== null) window.clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(async () => {
      timerRef.current = null;
      if (runningRef.current) {
        pendingRef.current = true;
        return;
      }
      runningRef.current = true;
      try {
        const status = await getSyncStatus();
        if (!status.syncEnabled || !status.authenticated) {
          retryDelayRef.current = 2_000;
          return;
        }
        const result = await syncNow();
        retryDelayRef.current = result.success
          ? 2_000
          : Math.min(retryDelayRef.current * 2, 5 * 60_000);
        if (!result.success) scheduleAutomaticSync(retryDelayRef.current);
      } catch {
        retryDelayRef.current = Math.min(retryDelayRef.current * 2, 5 * 60_000);
        scheduleAutomaticSync(retryDelayRef.current);
      } finally {
        runningRef.current = false;
        if (pendingRef.current) {
          pendingRef.current = false;
          scheduleAutomaticSync();
        }
      }
    }, delay);
  }, []);

  const cancelAutomaticSync = useCallback(() => {
    if (timerRef.current !== null) window.clearTimeout(timerRef.current);
    timerRef.current = null;
    pendingRef.current = false;
    retryDelayRef.current = 2_000;
  }, []);

  useEffect(() => {
    if (!isNativeApp()) return;
    const onResume = () => {
      if (document.visibilityState === 'visible') scheduleAutomaticSync();
    };
    const onOnline = () => scheduleAutomaticSync();
    document.addEventListener('visibilitychange', onResume);
    window.addEventListener('online', onOnline);
    return () => {
      document.removeEventListener('visibilitychange', onResume);
      window.removeEventListener('online', onOnline);
      cancelAutomaticSync();
    };
  }, [cancelAutomaticSync, scheduleAutomaticSync]);

  return { scheduleAutomaticSync, cancelAutomaticSync };
}
