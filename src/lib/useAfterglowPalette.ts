/**
 * Afterglow palette: derives three accent swatches from the current artwork.
 *
 * Rules from the design spec:
 *  - Never invent colour: if extraction is unavailable (missing artwork,
 *    decoded image blocked by the asset protocol, near-grey cover) the static
 *    sodium / violet / lagoon fallbacks apply.
 *  - Clamp OKLCH lightness to 0.55-0.80 and chroma to 0.08-0.22 so glows stay
 *    rich, never neon or muddy.
 *
 * Colour math is OKLab (Bjorn Ottosson). Canvas readback can throw
 * SecurityError on a tainted canvas; every failure path is non-throwing.
 */

import { useEffect } from 'react';

export interface ArtPalette {
  art1: string;
  art2: string;
  art3: string;
}

const FALLBACK: ArtPalette = {
  art1: '#ffb23e',
  art2: '#6a5acd',
  art3: '#4fe3c1',
};

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

function srgbToLinear(channel: number): number {
  const value = channel / 255;
  return value <= 0.04045 ? value / 12.92 : Math.pow((value + 0.055) / 1.055, 2.4);
}

function linearToSrgb(value: number): number {
  const clamped = clamp(value, 0, 1);
  const encoded = clamped <= 0.0031308 ? 12.92 * clamped : 1.055 * Math.pow(clamped, 1 / 2.4) - 0.055;
  return Math.round(clamp(encoded, 0, 1) * 255);
}

interface Oklch {
  l: number;
  c: number;
  h: number;
}

function rgbToOklch(r: number, g: number, b: number): Oklch {
  const lr = srgbToLinear(r);
  const lg = srgbToLinear(g);
  const lb = srgbToLinear(b);

  const l = 0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb;
  const m = 0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb;
  const s = 0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb;

  const l_ = Math.cbrt(l);
  const m_ = Math.cbrt(m);
  const s_ = Math.cbrt(s);

  const okL = 0.2104542553 * l_ + 0.793617785 * m_ - 0.0040720468 * s_;
  const okA = 1.9779984951 * l_ - 2.428592205 * m_ + 0.4505937099 * s_;
  const okB = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.808675766 * s_;

  return {
    l: okL,
    c: Math.hypot(okA, okB),
    h: Math.atan2(okB, okA),
  };
}

function oklchToRgb({ l, c, h }: Oklch): { r: number; g: number; b: number } {
  const a = c * Math.cos(h);
  const b = c * Math.sin(h);

  const l_ = l + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = l - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = l - 0.0894841775 * a - 1.291485548 * b;

  const ll = l_ * l_ * l_;
  const mm = m_ * m_ * m_;
  const ss = s_ * s_ * s_;

  return {
    r: linearToSrgb(4.0767416621 * ll - 3.3077115913 * mm + 0.2309699292 * ss),
    g: linearToSrgb(-1.2684380046 * ll + 2.6097574011 * mm - 0.3413193965 * ss),
    b: linearToSrgb(-0.0041960863 * ll - 0.7034186147 * mm + 1.707614701 * ss),
  };
}

function toHex({ r, g, b }: { r: number; g: number; b: number }): string {
  const hex = (value: number) => value.toString(16).padStart(2, '0');
  return `#${hex(r)}${hex(g)}${hex(b)}`;
}

/** Clamp a swatch into the Afterglow glow range. */
function clampSwatch(r: number, g: number, b: number): string {
  const oklch = rgbToOklch(r, g, b);
  return toHex(
    oklchToRgb({
      l: clamp(oklch.l, 0.55, 0.8),
      c: clamp(oklch.c, 0.08, 0.22),
      h: oklch.h,
    })
  );
}

interface Bucket {
  weight: number;
  r: number;
  g: number;
  b: number;
}

/**
 * Draw the artwork small, bucket the pixels and keep the three most distinct
 * chromatic swatches. Returns null when there is nothing usable to sample.
 */
function samplePalette(image: HTMLImageElement): ArtPalette | null {
  const canvas = document.createElement('canvas');
  const size = 32;
  canvas.width = size;
  canvas.height = size;

  const context = canvas.getContext('2d', { willReadFrequently: true });
  if (!context) return null;

  context.drawImage(image, 0, 0, size, size);

  let data: Uint8ClampedArray;
  try {
    data = context.getImageData(0, 0, size, size).data;
  } catch {
    // Tainted canvas (asset protocol without CORS): keep the brand fallbacks.
    return null;
  }

  const buckets = new Map<number, Bucket>();
  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] < 200) continue;
    const r = data[i];
    const g = data[i + 1];
    const b = data[i + 2];
    const key = ((r >> 5) << 10) | ((g >> 5) << 5) | (b >> 5);
    const existing = buckets.get(key);
    if (existing) {
      existing.weight += 1;
      existing.r += r;
      existing.g += g;
      existing.b += b;
    } else {
      buckets.set(key, { weight: 1, r, g, b });
    }
  }

  const candidates = [...buckets.values()]
    .map((bucket) => {
      const r = bucket.r / bucket.weight;
      const g = bucket.g / bucket.weight;
      const b = bucket.b / bucket.weight;
      const oklch = rgbToOklch(r, g, b);
      return { r, g, b, ...oklch, weight: bucket.weight };
    })
    // Drop near-black, near-white and near-grey pixels: they carry no hue.
    .filter((candidate) => candidate.l > 0.18 && candidate.l < 0.94 && candidate.c > 0.035)
    .sort((a, b) => b.weight - a.weight)
    .slice(0, 24);

  if (candidates.length === 0) return null;

  const distance = (a: Oklch, b: Oklch) => Math.abs(a.l - b.l) * 2 + Math.hypot(a.c * Math.cos(a.h) - b.c * Math.cos(b.h), a.c * Math.sin(a.h) - b.c * Math.sin(b.h)) * 2;

  const chosen = [candidates[0]];
  while (chosen.length < 3 && chosen.length < candidates.length) {
    let best = candidates[0];
    let bestScore = -1;
    for (const candidate of candidates) {
      if (chosen.includes(candidate)) continue;
      const score = Math.min(...chosen.map((picked) => distance(candidate, picked)));
      if (score > bestScore) {
        bestScore = score;
        best = candidate;
      }
    }
    chosen.push(best);
  }

  const hexes = chosen.map((candidate) => clampSwatch(candidate.r, candidate.g, candidate.b));
  return {
    art1: hexes[0] ?? FALLBACK.art1,
    art2: hexes[1] ?? FALLBACK.art2,
    art3: hexes[2] ?? hexes[1] ?? FALLBACK.art3,
  };
}

function applyPalette(palette: ArtPalette) {
  const root = document.documentElement;
  root.style.setProperty('--art-1', palette.art1);
  root.style.setProperty('--art-2', palette.art2);
  root.style.setProperty('--art-3', palette.art3);
}

function clearPalette() {
  const root = document.documentElement;
  root.style.removeProperty('--art-1');
  root.style.removeProperty('--art-2');
  root.style.removeProperty('--art-3');
}

/** Publish art tokens for the current artwork. Always resolves to *some* palette. */
export function useAfterglowPalette(src?: string) {
  useEffect(() => {
    if (!src) {
      clearPalette();
      return;
    }

    let cancelled = false;
    const image = new Image();
    image.crossOrigin = 'anonymous';
    image.decoding = 'async';
    image.onload = () => {
      if (cancelled) return;
      const palette = samplePalette(image);
      if (palette) applyPalette(palette);
      else clearPalette();
    };
    image.onerror = () => {
      if (!cancelled) clearPalette();
    };
    image.src = src;

    return () => {
      cancelled = true;
    };
  }, [src]);
}

export { FALLBACK as AFTERGLOW_FALLBACK_PALETTE };
