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

test('Cavia Connect exposes permanent Connect IDs, requests, ignored accounts, saved hosts, and independent controls', () => {
  const drawerSrc = fs.readFileSync(
    path.join(repoRoot, 'src/features/connect/ConnectDrawer.tsx'),
    'utf8'
  );
  const libSrc = fs.readFileSync(path.join(repoRoot, 'src/lib/connect.ts'), 'utf8');
  const settingsSrc = fs.readFileSync(
    path.join(repoRoot, 'src/features/settings/SettingsView.tsx'),
    'utf8'
  );

  for (const fnName of [
    'connectGetAccountHub',
    'connectUpdateAccountSettings',
    'connectLookupHost',
    'connectSendJoinRequest',
    'connectRespondRequest',
    'connectUnignoreGuest',
    'connectSaveHost',
    'connectRemoveSavedHost',
    'connectToSavedHost',
    'connectJoinApprovedSession',
  ]) {
    assert.match(libSrc, new RegExp(`export async function ${fnName}\\(`));
  }

  // Independent host connection controls (hosting vs acceptJoinRequests vs showConnectId)
  assert.match(drawerSrc, /Hosting Session/);
  assert.match(drawerSrc, /Accept Join Requests/);
  assert.match(drawerSrc, /Show Connect ID/);
  assert.match(drawerSrc, /handleToggleAcceptRequests/);
  assert.match(drawerSrc, /handleToggleShowConnectId/);

  // Permanent ID lookup, independent Save Host, Send Join Request, Saved Hosts
  assert.match(drawerSrc, /FIND HOST BY PERMANENT ID/);
  assert.match(drawerSrc, /Save Host/);
  assert.match(drawerSrc, /Send Join Request/);
  assert.match(drawerSrc, /SAVED HOSTS/);
  assert.match(drawerSrc, /Join Session/);

  // Incoming Request actions: Accept, Decline, Ignore 1d + Ignored Accounts Unignore
  assert.match(drawerSrc, /handleRespondRequest\(req\.requestId, 'accept'\)/);
  assert.match(drawerSrc, /handleRespondRequest\(req\.requestId, 'decline'\)/);
  assert.match(drawerSrc, /handleRespondRequest\(req\.requestId, 'ignore_1d'\)/);
  assert.match(drawerSrc, /IGNORED ACCOUNTS/);
  assert.match(drawerSrc, /Unignore/);

  // Settings View controls, Profile login, Library cache/preload controls & no Supabase jargon
  assert.match(settingsSrc, /label: 'Profile'/);
  assert.doesNotMatch(settingsSrc, /supabase/i);
  assert.doesNotMatch(settingsSrc, /sync-backend-url/);
  assert.match(settingsSrc, /library-preload-toggle/);
  assert.match(settingsSrc, /Remove all downloads/);
  assert.match(settingsSrc, /Clear artwork cache/);
  assert.match(settingsSrc, /Clear lyrics cache/);
  assert.match(settingsSrc, /connect-accept-requests/);
  assert.match(settingsSrc, /connect-show-id-meta/);
  assert.match(settingsSrc, /Ignored Accounts/);
  assert.match(settingsSrc, /handleSettingsUnignore/);
  assert.match(drawerSrc, /Settings &rarr; Profile/);
});
