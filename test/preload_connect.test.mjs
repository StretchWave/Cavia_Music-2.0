import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';

const repoRoot = path.resolve(import.meta.dirname, '..');

test('PreloadPill enforces 0-20 bounds and hides from Now Playing when disabled (0)', () => {
  const pillSrc = fs.readFileSync(
    path.join(repoRoot, 'src/features/player/PreloadPill.tsx'),
    'utf8'
  );
  const libSrc = fs.readFileSync(path.join(repoRoot, 'src/lib/connect.ts'), 'utf8');

  assert.match(libSrc, /export const MAX_PRELOAD_TARGET = 20/);
  assert.match(libSrc, /Math\.max\(0, Math\.min\(MAX_PRELOAD_TARGET, Math\.round\(target\)\)\)/);
  assert.match(
    pillSrc,
    /if \(!status \|\| !status\.enabled \|\| status\.targetCount <= 0\) \{[\s\S]*return null;/
  );
  assert.match(pillSrc, /\{readyCount\}\/\{targetCount\} ready/);
});

test('Now Playing integrates compact PreloadPill and Queue/Settings expose re-enable controls', () => {
  const dockSrc = fs.readFileSync(
    path.join(repoRoot, 'src/features/player/PlayerDock.tsx'),
    'utf8'
  );
  const expandedSrc = fs.readFileSync(
    path.join(repoRoot, 'src/features/player/ExpandedPlayer.tsx'),
    'utf8'
  );
  const queueSrc = fs.readFileSync(
    path.join(repoRoot, 'src/features/player/QueueDrawer.tsx'),
    'utf8'
  );
  const settingsSrc = fs.readFileSync(
    path.join(repoRoot, 'src/features/settings/SettingsView.tsx'),
    'utf8'
  );
  const appSrc = fs.readFileSync(path.join(repoRoot, 'src/app/App.tsx'), 'utf8');

  assert.match(dockSrc, /<PreloadPill/);
  assert.match(expandedSrc, /<PreloadPill/);
  assert.match(queueSrc, /queue-preload-stepper/);
  assert.match(settingsSrc, /Queue preload window/);
  assert.match(appSrc, /Internet is unavailable\. Would you like to continue with cached songs\?/);
  assert.match(appSrc, /Shuffle Cached Songs/);
  assert.match(appSrc, /<ConnectDrawer/);

  const importsSrc = fs.readFileSync(
    path.join(repoRoot, 'src/features/imports/ImportsView.tsx'),
    'utf8'
  );
  const afterglowCss = fs.readFileSync(
    path.join(repoRoot, 'src/design/afterglow.css'),
    'utf8'
  );
  assert.match(importsSrc, /onExit\?: \(\) => void;/);
  assert.match(importsSrc, />[\s\n]*Exit[\s\n]*<\/Button>/);
  assert.match(afterglowCss, /\.cavia-sidebar:has\(:focus-visible\)/);
  assert.doesNotMatch(afterglowCss, /\.cavia-sidebar:focus-within/);
});
