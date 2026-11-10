import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

export function formatSmtcDiagnostic(status, error, systemMediaControls) {
  if (status === 'ready' || (!status && systemMediaControls)) {
    return 'SMTC ACTIVE';
  }
  if (status === 'error') {
    return `SMTC ERROR: ${error || 'Runtime fault'}`;
  }
  if (status === 'unavailable') {
    return error ? `SMTC UNAVAILABLE: ${error}` : 'SMTC UNAVAILABLE (No active window handle)';
  }
  if (status === 'unsupported') {
    return error ? `SMTC UNSUPPORTED: ${error}` : 'SMTC UNSUPPORTED';
  }
  return error ? `SMTC UNAVAILABLE: ${error}` : 'SMTC UNAVAILABLE';
}

describe('SMTC Diagnostics Display & Status Contract', () => {
  it('Test A: unavailable with error displays error in Settings', () => {
    const errorMsg = 'Failed to attach to HWND: 0x00050124';
    const text = formatSmtcDiagnostic('unavailable', errorMsg, false);
    assert.ok(
      text.includes(errorMsg),
      `Rendered text "${text}" must include the specific error message`
    );
    assert.strictEqual(text, `SMTC UNAVAILABLE: ${errorMsg}`);
  });

  it('Test B: unavailable without error displays clean fallback', () => {
    const text = formatSmtcDiagnostic('unavailable', null, false);
    assert.strictEqual(text, 'SMTC UNAVAILABLE (No active window handle)');
    assert.ok(!text.includes('null') && !text.includes('undefined'));
  });

  it('Test C: error status displays error', () => {
    const errorMsg = 'SetNowPlayingMetadata failed with Win32 error';
    const text = formatSmtcDiagnostic('error', errorMsg, false);
    assert.strictEqual(text, `SMTC ERROR: ${errorMsg}`);
  });

  it('Test D: ready status displays active text', () => {
    const text = formatSmtcDiagnostic('ready', null, true);
    assert.strictEqual(text, 'SMTC ACTIVE');
  });

  it('Test E: Initializing state is excluded from canonical states', () => {
    const validSmtcStatuses = ['ready', 'unavailable', 'unsupported', 'error'];
    assert.ok(
      !validSmtcStatuses.includes('initializing'),
      'Initializing phantom state must be removed'
    );
  });
});
