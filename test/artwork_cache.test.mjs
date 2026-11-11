import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

export function formatBytes(bytes) {
  if (bytes <= 0 || isNaN(bytes)) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const val = bytes / Math.pow(k, i);
  return `${val.toFixed(i === 0 ? 0 : 1)} ${sizes[i]}`;
}

export function isValidArtworkFilename(name) {
  if (!name || name.startsWith('.')) return false;
  const lastDot = name.lastIndexOf('.');
  if (lastDot <= 0) return false;
  const ext = name.slice(lastDot + 1).toLowerCase();
  const hash = name.slice(0, lastDot);
  if (hash.length !== 64 || !/^[0-9a-fA-F]+$/.test(hash)) return false;
  return ['jpg', 'jpeg', 'png', 'webp', 'gif'].includes(ext);
}

describe('Artwork Cache — FormatBytes Utility', () => {
  it('formats zero and negative bytes safely as 0 B', () => {
    assert.strictEqual(formatBytes(0), '0 B');
    assert.strictEqual(formatBytes(-100), '0 B');
    assert.strictEqual(formatBytes(NaN), '0 B');
  });

  it('formats bytes in B, KB, MB, GB ranges accurately', () => {
    assert.strictEqual(formatBytes(512), '512 B');
    assert.strictEqual(formatBytes(1024), '1.0 KB');
    assert.strictEqual(formatBytes(1536), '1.5 KB');
    assert.strictEqual(formatBytes(1048576), '1.0 MB');
    assert.strictEqual(formatBytes(1073741824), '1.0 GB');
  });
});

describe('Artwork Cache — Filename Validation Contract', () => {
  it('accepts 64-hex SHA-256 filenames with valid image extensions', () => {
    const valid = [
      'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.jpg',
      '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.png',
      '0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF.webp',
      'abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789.gif',
      'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.jpeg',
    ];
    for (const name of valid) {
      assert.strictEqual(isValidArtworkFilename(name), true, `Expected valid: ${name}`);
    }
  });

  it('rejects temporary files, short names, non-artwork extensions, and directories', () => {
    const invalid = [
      '.tmp_e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.jpg',
      'short.jpg',
      'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.txt',
      'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855.mp3',
      '',
      'noextension',
      '.DS_Store',
      'Thumbs.db',
    ];
    for (const name of invalid) {
      assert.strictEqual(isValidArtworkFilename(name), false, `Expected invalid: ${name}`);
    }
  });
});

describe('Artwork Cache — DTO Contract Structure', () => {
  it('validates ArtworkCacheSummary camelCase structure and invariant math', () => {
    const summary = {
      totalBytes: 5242880,
      totalFiles: 10,
      referencedFiles: 8,
      orphanedFiles: 2,
      reclaimableBytes: 1048576,
      missingReferencedFiles: 0,
      cacheDirExists: true,
      cacheDirPath: 'C:\\Users\\User\\AppData\\Roaming\\com.cavia.music\\artworks',
      inspectionErrors: [],
      formatCounts: { jpg: 8, png: 2 },
    };

    assert.strictEqual(typeof summary.totalBytes, 'number');
    assert.strictEqual(typeof summary.totalFiles, 'number');
    assert.strictEqual(typeof summary.referencedFiles, 'number');
    assert.strictEqual(typeof summary.orphanedFiles, 'number');
    assert.strictEqual(typeof summary.reclaimableBytes, 'number');
    assert.strictEqual(typeof summary.missingReferencedFiles, 'number');
    assert.strictEqual(typeof summary.cacheDirExists, 'boolean');
    assert.strictEqual(typeof summary.cacheDirPath, 'string');
    assert.ok(Array.isArray(summary.inspectionErrors));
    assert.strictEqual(typeof summary.formatCounts, 'object');

    // Invariants
    assert.strictEqual(summary.referencedFiles + summary.orphanedFiles, summary.totalFiles);
    assert.ok(summary.reclaimableBytes <= summary.totalBytes);
  });

  it('validates ArtworkCleanupResult and ArtworkRecoveryResult structure', () => {
    const cleanup = {
      deletedFiles: 3,
      reclaimedBytes: 1572864,
      skippedFiles: 0,
      failures: [],
    };
    assert.strictEqual(cleanup.deletedFiles, 3);
    assert.strictEqual(cleanup.reclaimedBytes, 1572864);
    assert.strictEqual(cleanup.skippedFiles, 0);
    assert.ok(Array.isArray(cleanup.failures));

    const recovery = {
      totalChecked: 2,
      recoveredTracks: 2,
      alreadyValidTracks: 0,
      missingAudioFiles: 0,
      noEmbeddedArtwork: 0,
      failedTracks: 0,
      reclaimedOrWrittenBytes: 1048576,
      errors: [],
    };
    assert.strictEqual(recovery.totalChecked, 2);
    assert.strictEqual(recovery.recoveredTracks, 2);
    assert.ok(Array.isArray(recovery.errors));
  });
});
