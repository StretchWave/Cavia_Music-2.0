/**
 * Reference-counted overlay stack for modals, drawers, and full-screen overlays.
 * Prevents nested overlays (e.g., confirmation dialogs inside PlaylistDetailModal)
 * from prematurely unlocking body scroll or allowing Escape to close multiple layers.
 */
const overlayStack: string[] = [];

export function acquireOverlayLock(id: string): void {
  const existingIdx = overlayStack.indexOf(id);
  if (existingIdx !== -1) {
    overlayStack.splice(existingIdx, 1);
  }
  overlayStack.push(id);
  if (typeof document !== 'undefined') {
    document.body.style.overflow = 'hidden';
  }
}

export function releaseOverlayLock(id: string): void {
  const idx = overlayStack.indexOf(id);
  if (idx !== -1) {
    overlayStack.splice(idx, 1);
  }
  if (typeof document !== 'undefined' && overlayStack.length === 0) {
    document.body.style.overflow = '';
  }
}

export function isTopOverlay(id: string): boolean {
  return overlayStack.length > 0 && overlayStack[overlayStack.length - 1] === id;
}
