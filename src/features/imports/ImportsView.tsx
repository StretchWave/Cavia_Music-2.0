import { useEffect, useState, type FormEvent } from 'react';
import {
  DownloadCloud,
  FolderDown,
  FolderPlus,
  Loader2,
  Pause,
  Play,
  Trash2,
  X,
  ExternalLink,
} from 'lucide-react';
import { Button } from '../../components/Button';
import type { ScanProgressEvent } from '../../lib/library';
import {
  clearCompletedImports,
  getImportJobs,
  listenImportProgress,
  pauseImport,
  resumeImport,
  cancelImport,
  removeImportJob,
  cancelAndDeleteImport,
  startPlaylistImport,
  type ImportJob,
} from '../../lib/spotify';
import { detectPlaylistImportSource } from '../../lib/trackMatching';

interface ImportsViewProps {
  onOpenPlaylist: (playlistId: string) => void;
  onExit?: () => void;
  onPlaylistCreated?: () => void;
  onImportFolder?: () => void;
  isScanning?: boolean;
  scanProgress?: ScanProgressEvent | null;
  onCancelScan?: () => void;
}

export function ImportsView({
  onOpenPlaylist,
  onExit,
  onPlaylistCreated,
  onImportFolder,
  isScanning = false,
  scanProgress = null,
  onCancelScan,
}: ImportsViewProps) {
  const [jobs, setJobs] = useState<ImportJob[]>([]);
  const [url, setUrl] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const detectedSource = detectPlaylistImportSource(url);

  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;

    getImportJobs().then(setJobs).catch(console.error);

    listenImportProgress((updatedJob) => {
      setJobs((prev) => {
        const index = prev.findIndex((j) => j.id === updatedJob.id);
        if (index === -1) {
          return [updatedJob, ...prev];
        }
        const next = [...prev];
        next[index] = updatedJob;
        return next;
      });
    }).then((unlisten) => {
      unlistenProgress = unlisten;
    }).catch(console.error);

    return () => {
      unlistenProgress?.();
    };
  }, []);

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault();
    const link = url.trim();
    if (!link || isSubmitting) return;

    setIsSubmitting(true);
    setError(null);

    try {
      const job = await startPlaylistImport(link);
      setJobs((prev) => [job, ...prev]);
      setUrl('');
      onPlaylistCreated?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSubmitting(false);
    }
  };

  const handlePause = async (jobId: string) => {
    try {
      await pauseImport(jobId);
    } catch (err) {
      console.error('Failed to pause import:', err);
    }
  };

  const handleResume = async (jobId: string) => {
    try {
      await resumeImport(jobId);
    } catch (err) {
      console.error('Failed to resume import:', err);
    }
  };

  const handleCancel = async (jobId: string) => {
    try {
      await cancelImport(jobId);
    } catch (err) {
      console.error('Failed to cancel import:', err);
    }
  };

  const handleCancelAndDelete = async (jobId: string) => {
    try {
      await cancelAndDeleteImport(jobId);
      setJobs((prev) => prev.filter((j) => j.id !== jobId));
      onPlaylistCreated?.();
    } catch (err) {
      console.error('Failed to cancel and delete import:', err);
    }
  };

  const handleRemoveJob = async (jobId: string) => {
    try {
      await removeImportJob(jobId);
      setJobs((prev) => prev.filter((j) => j.id !== jobId));
    } catch (err) {
      console.error('Failed to remove import job:', err);
    }
  };

  const handleClearCompleted = async () => {
    try {
      await clearCompletedImports();
      setJobs((prev) => prev.filter((j) => j.status === 'running' || j.status === 'paused'));
    } catch (err) {
      console.error('Failed to clear completed imports:', err);
    }
  };

  const activeJobs = jobs.filter((j) => j.status === 'running' || j.status === 'paused' || j.status === 'queued');
  const pastJobs = jobs.filter((j) => j.status === 'completed' || j.status === 'cancelled' || j.status === 'failed');
  const scanPercent =
    scanProgress && scanProgress.discoveredCount > 0
      ? Math.min(100, Math.round((scanProgress.processedCount / scanProgress.discoveredCount) * 100))
      : 0;

  return (
    <section className="imports-view" aria-label="Music and Playlist Imports">
      <header className="imports-header">
        <div className="imports-title-group">
          <h1 className="imports-heading">Imports</h1>
          <p className="imports-subheading">
            Add local music folders or import public Spotify and YouTube Music playlists with live background progress.
          </p>
        </div>

        <div className="imports-header-actions">
          {pastJobs.length > 0 && (
            <Button
              variant="ghost"
              size="sm"
              icon={<Trash2 size={14} />}
              onClick={handleClearCompleted}
              title="Clear finished and cancelled jobs from this view"
            >
              Clear finished
            </Button>
          )}
          {onExit && (
            <Button
              variant="outline"
              size="sm"
              icon={<X size={14} />}
              onClick={onExit}
              aria-label="Exit imports back to library"
              title="Exit Imports and return to Library"
            >
              Exit
            </Button>
          )}
        </div>
      </header>

      {/* Local Folder Scan Card */}
      <div className="imports-input-card imports-local-card">
        <div className="import-card-top">
          <div className="import-card-info">
            <h2 className="import-card-title">Local Music Folders</h2>
            <p className="imports-note">
              Index audio files directly from your device folders (MP3, FLAC, M4A, AAC, OGG, Opus, WAV, AIFF).
            </p>
          </div>
          {onImportFolder && (
            <Button
              variant={isScanning ? 'secondary' : 'outline'}
              size="sm"
              icon={isScanning ? <Loader2 size={14} className="spinning" /> : <FolderPlus size={14} />}
              onClick={onImportFolder}
              disabled={isScanning}
            >
              {isScanning ? 'Scanning…' : 'Import folder'}
            </Button>
          )}
        </div>

        {isScanning && scanProgress && (
          <div className="imports-local-progress" role="status" aria-live="polite">
            <div className="import-progress-container">
              <div className="import-progress-bar">
                <div
                  className="import-progress-fill animate-progress"
                  style={{ width: `${Math.max(4, scanPercent)}%` }}
                />
              </div>
              <span className="import-progress-percent">{scanPercent}%</span>
              {onCancelScan && (
                <Button variant="ghost" size="sm" icon={<X size={13} />} onClick={onCancelScan}>
                  Cancel
                </Button>
              )}
            </div>
            <div className="import-card-meta">
              {scanProgress.phase === 'discovering'
                ? 'Discovering audio files…'
                : `${scanProgress.processedCount} / ${scanProgress.discoveredCount} files · ${scanProgress.newTracksCount} added · ${scanProgress.duplicatesCount} existing`}
            </div>
            {scanProgress.currentFile && (
              <div className="import-current-track" title={scanProgress.currentFile}>
                <code>{scanProgress.currentFile}</code>
              </div>
            )}
          </div>
        )}

        {!isScanning && scanProgress && (scanProgress.phase === 'completed' || scanProgress.phase === 'cancelled') && (
          <p className="imports-note" role="status">
            Last folder scan {scanProgress.phase === 'cancelled' ? 'cancelled' : 'completed'}:{' '}
            {scanProgress.newTracksCount} added · {scanProgress.duplicatesCount} existing ·{' '}
            {scanProgress.errorsCount} errors
          </p>
        )}
      </div>

      {/* Playlist Import Form */}
      <div className="imports-input-card">
        <form onSubmit={handleSubmit} className="imports-form">
          <div className="imports-form-header">
            <h2 className="import-card-title">Spotify &amp; YouTube Music Playlists</h2>
            {detectedSource && (
              <span className={`imports-source-pill source-${detectedSource}`}>
                {detectedSource === 'youtube_music' ? 'YouTube Music link' : 'Spotify link'}
              </span>
            )}
          </div>
          <div className="imports-input-wrapper">
            <input
              type="text"
              placeholder="Paste public Spotify or YouTube Music playlist link (https://open.spotify.com/playlist/... or https://music.youtube.com/playlist?list=...)"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              className="cavia-text-input imports-text-input"
              disabled={isSubmitting}
              aria-label="Spotify or YouTube Music playlist URL"
            />
            <Button
              type="submit"
              variant="primary"
              size="sm"
              icon={isSubmitting ? <Loader2 size={14} className="spinning" /> : <DownloadCloud size={14} />}
              disabled={!url.trim() || isSubmitting}
            >
              {isSubmitting ? 'Starting…' : 'Import'}
            </Button>
          </div>
          {error && (
            <div className="cavia-banner banner-error imports-error-banner" role="alert">
              {error}
            </div>
          )}
          <p className="imports-note">
            Public playlists are supported up to 1,000 tracks. Local library matches are linked first;
            YouTube Music imports become playable immediately while local matches finish in the background.
          </p>
        </form>
      </div>

      {/* Active Jobs */}
      {activeJobs.length > 0 && (
        <div className="imports-section">
          <h2 className="imports-section-title">Active ({activeJobs.length})</h2>
          <div className="imports-grid">
            {activeJobs.map((job) => {
              const progressPct = job.total > 0 ? Math.round((job.processed / job.total) * 100) : 0;
              const isRunning = job.status === 'running';
              const isPaused = job.status === 'paused';
              const sourceLabel = job.source === 'youtube_music' ? 'YouTube Music' : 'Spotify';

              return (
                <div key={job.id} className="import-card active-job-card">
                  <div className="import-card-top">
                    <div className="import-card-info">
                      <div className="import-card-title-row">
                        <h3 className="import-card-title">{job.playlistName}</h3>
                        <span className={`imports-source-pill source-${job.source || 'spotify'}`}>
                          {sourceLabel}
                        </span>
                      </div>
                      <span className="import-card-meta">
                        {job.total} tracks · {job.matchedLocal} local · {job.matchedOnline} online · {job.skipped} skipped
                      </span>
                    </div>
                    <span className={`import-badge badge-${job.status}`}>
                      {isRunning && <span className="pulsing-dot" />}
                      {job.status}
                    </span>
                  </div>

                  {/* Progress Bar */}
                  <div className="import-progress-container">
                    <div className="import-progress-bar">
                      <div
                        className={`import-progress-fill ${isRunning ? 'animate-progress' : ''}`}
                        style={{ width: `${progressPct}%` }}
                      />
                    </div>
                    <span className="import-progress-percent">{progressPct}%</span>
                  </div>

                  {job.notice && (
                    <p className="imports-note import-job-notice" role="status">
                      {job.notice}
                    </p>
                  )}

                  {/* Live Status Track */}
                  {job.currentTrack && isRunning && (
                    <div className="import-current-track" title={job.currentTrack}>
                      <Loader2 size={12} className="spinning" />
                      <span>Matching: {job.currentTrack}</span>
                    </div>
                  )}

                  {/* Controls */}
                  <div className="import-card-actions">
                    {isRunning && (
                      <Button
                        variant="secondary"
                        size="sm"
                        icon={<Pause size={13} />}
                        onClick={() => handlePause(job.id)}
                      >
                        Pause
                      </Button>
                    )}
                    {isPaused && (
                      <Button
                        variant="primary"
                        size="sm"
                        icon={<Play size={13} />}
                        onClick={() => handleResume(job.id)}
                      >
                        Resume
                      </Button>
                    )}
                    <Button
                      variant="ghost"
                      size="sm"
                      icon={<X size={13} />}
                      onClick={() => handleCancel(job.id)}
                      title="Cancel background matching"
                    >
                      Cancel
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="text-danger"
                      icon={<Trash2 size={13} />}
                      onClick={() => handleCancelAndDelete(job.id)}
                      title="Cancel import and delete created playlist"
                    >
                      Cancel & Delete
                    </Button>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Completed & Past Jobs */}
      {pastJobs.length > 0 && (
        <div className="imports-section">
          <h2 className="imports-section-title">Past Imports</h2>
          <div className="imports-grid">
            {pastJobs.map((job) => {
              const isCompleted = job.status === 'completed';
              const sourceLabel = job.source === 'youtube_music' ? 'YouTube Music' : 'Spotify';

              return (
                <div key={job.id} className="import-card past-job-card">
                  <div className="import-card-top">
                    <div className="import-card-info">
                      <div className="import-card-title-row">
                        <h3 className="import-card-title">{job.playlistName}</h3>
                        <span className={`imports-source-pill source-${job.source || 'spotify'}`}>
                          {sourceLabel}
                        </span>
                      </div>
                      <span className="import-card-meta">
                        {isCompleted ? (
                          <>
                            {job.matchedLocal + job.matchedOnline} of {job.total} tracks resolved
                            {job.skipped > 0 ? ` · ${job.skipped} skipped` : ''}
                          </>
                        ) : (
                          <>Import {job.status}</>
                        )}
                      </span>
                    </div>
                    <span className={`import-badge badge-${job.status}`}>
                      {job.status}
                    </span>
                  </div>

                  {job.notice && (
                    <p className="imports-note import-job-notice" role="status">
                      {job.notice}
                    </p>
                  )}

                  <div className="import-card-actions">
                    <Button
                      variant="secondary"
                      size="sm"
                      icon={<ExternalLink size={13} />}
                      onClick={() => onOpenPlaylist(job.playlistId)}
                    >
                      View Playlist
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      icon={<Trash2 size={13} />}
                      onClick={() => handleRemoveJob(job.id)}
                      title="Remove import record"
                    >
                      Dismiss
                    </Button>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Empty State */}
      {jobs.length === 0 && !isScanning && (
        <div className="imports-empty-state">
          <div className="imports-empty-icon">
            <FolderDown size={36} strokeWidth={1.4} />
          </div>
          <h3 className="imports-empty-title">No active playlist imports</h3>
          <p className="imports-empty-desc">
            Choose a local music folder above or paste any public Spotify or YouTube Music playlist URL
            to bring your playlists into Cavia with live background progress.
          </p>
        </div>
      )}
    </section>
  );
}
