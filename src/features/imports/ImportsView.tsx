import { useEffect, useState, type FormEvent } from 'react';
import {
  DownloadCloud,
  FolderDown,
  Loader2,
  Pause,
  Play,
  Trash2,
  X,
  ExternalLink,
} from 'lucide-react';
import { Button } from '../../components/Button';
import {
  clearCompletedImports,
  getImportJobs,
  listenImportProgress,
  pauseImport,
  resumeImport,
  cancelImport,
  removeImportJob,
  cancelAndDeleteImport,
  startSpotifyImport,
  type ImportJob,
} from '../../lib/spotify';

interface ImportsViewProps {
  onOpenPlaylist: (playlistId: string) => void;
  onPlaylistCreated?: () => void;
}

export function ImportsView({ onOpenPlaylist, onPlaylistCreated }: ImportsViewProps) {
  const [jobs, setJobs] = useState<ImportJob[]>([]);
  const [url, setUrl] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
      const job = await startSpotifyImport(link);
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

  return (
    <section className="imports-view" aria-label="Playlist Imports">
      <header className="imports-header">
        <div className="imports-title-group">
          <h1 className="imports-heading">Imports</h1>
          <p className="imports-subheading">
            Background playlist imports with asynchronous catalogue resolution. Imports continue even when minimized.
          </p>
        </div>

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
      </header>

      {/* Import Form */}
      <div className="imports-input-card">
        <form onSubmit={handleSubmit} className="imports-form">
          <div className="imports-input-wrapper">
            <input
              type="text"
              placeholder="Paste public Spotify playlist link (https://open.spotify.com/playlist/...)"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              className="cavia-text-input imports-text-input"
              disabled={isSubmitting}
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
            Playlist metadata is read first, then streaming matches resolve in the background. Public
            playlists are supported up to 1,000 tracks; playlists longer than Spotify's embed payload
            can take a little longer before the job appears.
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

              return (
                <div key={job.id} className="import-card active-job-card">
                  <div className="import-card-top">
                    <div className="import-card-info">
                      <h3 className="import-card-title">{job.playlistName}</h3>
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

              return (
                <div key={job.id} className="import-card past-job-card">
                  <div className="import-card-top">
                    <div className="import-card-info">
                      <h3 className="import-card-title">{job.playlistName}</h3>
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
      {jobs.length === 0 && (
        <div className="imports-empty-state">
          <div className="imports-empty-icon">
            <FolderDown size={36} strokeWidth={1.4} />
          </div>
          <h3 className="imports-empty-title">No imports yet</h3>
          <p className="imports-empty-desc">
            Paste any public Spotify playlist URL into the box above to import it. Cavia reads the
            playlist, then resolves streaming matches in the background. Public playlists are supported
            up to 1,000 tracks.
          </p>
        </div>
      )}
    </section>
  );
}
