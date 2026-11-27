import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { extractShutdownErrorMessage } from '../src/lib/diagnostics.js';

function handleShutdownErrorPayload(payload, setError) {
  setError(extractShutdownErrorMessage(payload));
}

describe('Shutdown Error Event Contract', () => {
  it('fails to extract error message when backend emits bare string', () => {
    // Raw payload emitted by old backend:
    const bareStringPayload = JSON.parse(
      '"Failed to save playback state: database disk image is malformed (code 11)"'
    );
    let capturedError = 'sentinel';
    handleShutdownErrorPayload(bareStringPayload, (err) => {
      capturedError = err;
    });

    // In the old buggy implementation, capturedError becomes null:
    assert.strictEqual(
      capturedError,
      null,
      'Old bare-string payload resulted in null error, causing recovery modal not to render'
    );
  });

  it('correctly extracts error message when backend emits structured object', () => {
    // Structured payload emitted by corrected backend:
    const structuredPayload = JSON.parse(
      '{"error":"Failed to save playback state: database disk image is malformed (code 11)"}'
    );
    let capturedError = null;
    handleShutdownErrorPayload(structuredPayload, (err) => {
      capturedError = err;
    });

    assert.strictEqual(
      capturedError,
      'Failed to save playback state: database disk image is malformed (code 11)'
    );
  });
});
