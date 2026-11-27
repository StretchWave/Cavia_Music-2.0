import { useEffect, useId, useRef, useState } from 'react';
import {
  Check,
  ChevronDown,
  ChevronUp,
  Copy,
  Crown,
  ListPlus,
  Loader2,
  Plus,
  Radio,
  RefreshCw,
  Search,
  ShieldAlert,
  Trash2,
  Users,
  WifiOff,
  X,
} from 'lucide-react';
import { Button } from '../../components/Button';
import { Badge } from '../../components/Badge';
import { IconButton } from '../../components/IconButton';
import { Tabs } from '../../components/Tabs';
import { SearchInput } from '../../components/SearchInput';
import { acquireOverlayLock, isTopOverlay, releaseOverlayLock } from '../../lib/modalStack';
import { ytMusicTrackToTrack } from '../../lib/trackMatching';
import { searchYouTubeMusic, type YtMusicTrack } from '../../lib/ytmusic';
import {
  connectContributeTrack,
  connectContributeYtmusicPlaylist,
  connectCreateSession,
  connectEndSession,
  connectHostReorder,
  connectJoinSession,
  connectRemoveEntry,
  connectSetAlpha,
  connectSyncNow,
} from '../../lib/connect';
import type { ConnectSessionSnapshot, Track } from '../../lib/types';

interface ConnectDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  session: ConnectSessionSnapshot | null;
  isOffline: boolean;
  localTracks: Track[];
  onSessionChanged: (snapshot: ConnectSessionSnapshot | null) => void;
  onNotify?: (kind: 'success' | 'error' | 'info', message: string) => void;
}

type ConnectTab = 'queue' | 'contribute' | 'contributors';

export function ConnectDrawer({
  isOpen,
  onClose,
  session,
  isOffline,
  localTracks,
  onSessionChanged,
  onNotify,
}: ConnectDrawerProps) {
  const [tab, setTab] = useState<ConnectTab>('queue');
  const [setupMode, setSetupMode] = useState<'host' | 'join'>('host');
  const [sessionName, setSessionName] = useState('Cavia Listening Room');
  const [hostName, setHostName] = useState('Host');
  const [joinCodeInput, setJoinCodeInput] = useState('');
  const [guestName, setGuestName] = useState('Guest');
  const [alpha, setAlpha] = useState(1.0);
  const [busy, setBusy] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [copiedCode, setCopiedCode] = useState(false);

  // Contribute state
  const [searchQuery, setSearchQuery] = useState('');
  const [searching, setSearching] = useState(false);
  const [ytResults, setYtResults] = useState<YtMusicTrack[]>([]);
  const [playlistUrl, setPlaylistUrl] = useState('');
  const [priorityOverride, setPriorityOverride] = useState(false);

  const drawerRef = useRef<HTMLDivElement>(null);
  const overlayId = `connect-drawer-${useId()}`;
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useEffect(() => {
    if (!isOpen) return;
    acquireOverlayLock(overlayId);
    const handleKey = (e: KeyboardEvent) => {
      if (!isTopOverlay(overlayId)) return;
      if (e.key === 'Escape') {
        e.preventDefault();
        onCloseRef.current();
      }
    };
    window.addEventListener('keydown', handleKey);
    return () => {
      releaseOverlayLock(overlayId);
      window.removeEventListener('keydown', handleKey);
    };
  }, [isOpen, overlayId]);

  if (!isOpen) return null;

  const isHost = session?.localRole === 'host';

  const handleCreate = async () => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const snap = await connectCreateSession(sessionName, hostName, alpha);
      onSessionChanged(snap);
      onNotify?.('success', `Started Cavia Connect (${snap?.joinCode ?? ''})`);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleJoin = async () => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const snap = await connectJoinSession(joinCodeInput, guestName);
      onSessionChanged(snap);
      onNotify?.('success', `Joined ${snap?.name ?? 'Cavia Connect'}`);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleCopyCode = async () => {
    if (!session) return;
    try {
      await navigator.clipboard.writeText(session.joinCode);
      setCopiedCode(true);
      setTimeout(() => setCopiedCode(false), 1800);
    } catch {
      onNotify?.('info', `Join code: ${session.joinCode}`);
    }
  };

  const handleSearchYt = async (e?: React.FormEvent) => {
    e?.preventDefault();
    const q = searchQuery.trim();
    if (!q) return;
    setSearching(true);
    setErrorMsg(null);
    try {
      const results = await searchYouTubeMusic(q, `connect-${Date.now()}`);
      setYtResults(results);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setSearching(false);
    }
  };

  const handleContributeYt = async (item: YtMusicTrack) => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const res = await connectContributeTrack({
        track: ytMusicTrackToTrack(item),
        priorityOverride: isHost && priorityOverride,
      });
      if (res) {
        onSessionChanged(res.snapshot);
        setPriorityOverride(false);
        const statusMsg =
          res.entry.status === 'pending_preload'
            ? `"${item.title}" queued to preload & verify before entering sequence.`
            : `Added "${item.title}" to collaborative queue.`;
        onNotify?.('success', statusMsg);
      }
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleContributeLocal = async (track: Track) => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const res = await connectContributeTrack({
        track,
        priorityOverride: isHost && priorityOverride,
      });
      if (res) {
        onSessionChanged(res.snapshot);
        setPriorityOverride(false);
        onNotify?.('success', `Added "${track.title}" to session queue.`);
      }
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleContributePlaylist = async () => {
    const trimmed = playlistUrl.trim();
    if (!trimmed) return;
    setBusy(true);
    setErrorMsg(null);
    try {
      const res = await connectContributeYtmusicPlaylist({
        playlistUrlOrId: trimmed,
      });
      if (res) {
        onSessionChanged(res.snapshot);
        setPlaylistUrl('');
        onNotify?.(
          'success',
          `Added ${res.acceptedCount} tracks (${res.rejectedDuplicates} duplicates skipped)`
        );
      }
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleReorder = async (fromIndex: number, toIndex: number) => {
    try {
      const snap = await connectHostReorder(fromIndex, toIndex);
      onSessionChanged(snap);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    }
  };

  const handleRemove = async (queueEntryId: string) => {
    try {
      const snap = await connectRemoveEntry(queueEntryId, true);
      onSessionChanged(snap);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    }
  };

  const handleSync = async () => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const res = await connectSyncNow();
      if (res?.errorMessage) {
        setErrorMsg(res.errorMessage);
      } else if (res) {
        onNotify?.('info', `Synced ${res.pushedEvents} pending Connect events.`);
      }
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleAlphaChange = async (nextAlpha: number) => {
    try {
      const snap = await connectSetAlpha(nextAlpha);
      onSessionChanged(snap);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    }
  };

  const handleEndSession = async () => {
    setBusy(true);
    try {
      await connectEndSession();
      onSessionChanged(null);
      onNotify?.('info', 'Ended Cavia Connect session.');
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="cavia-drawer-backdrop" onClick={onClose}>
      <div
        ref={drawerRef}
        role="dialog"
        aria-modal="true"
        aria-label="Cavia Connect collaborative session"
        className="cavia-drawer-panel connect-drawer-panel"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="sheet-drag-handle" aria-hidden="true" />

        <div className="drawer-header">
          <div>
            <div className="drawer-title-row">
              <Radio size={17} className="text-vermilion" aria-hidden="true" />
              <h2 className="drawer-title">
                {session ? session.name : 'Cavia Connect'}
              </h2>
              {session && (
                <Badge variant={isHost ? 'vermilion' : 'neutral'} size="sm">
                  {isHost ? 'HOST' : 'GUEST'}
                </Badge>
              )}
            </div>
            {session && (
              <div className="connect-subbar">
                <button
                  type="button"
                  className="connect-code-pill"
                  onClick={handleCopyCode}
                  title="Copy session join code"
                >
                  <span>{session.joinCode}</span>
                  {copiedCode ? <Check size={12} /> : <Copy size={12} />}
                </button>
                <span className="connect-rev-text">
                  Rev {session.queueRevision}
                </span>
                {isOffline && (
                  <Badge variant="warn" size="sm">
                    <WifiOff size={11} /> Offline
                  </Badge>
                )}
              </div>
            )}
          </div>
          <IconButton
            label="Close Cavia Connect"
            icon={<X size={18} />}
            variant="ghost"
            onClick={onClose}
          />
        </div>

        <div className="drawer-body">
          {errorMsg && (
            <div className="modal-error-box connect-alert-box" role="alert">
              <ShieldAlert size={14} />
              <span>{errorMsg}</span>
            </div>
          )}

          {!session ? (
            <div className="connect-setup-stack">
              <p className="connect-intro-text">
                Listen fairly together. The host remains playback authority while contributors earn higher priority whenever they have fewer pending tracks.
              </p>

              <Tabs
                items={[
                  { id: 'host', label: 'Start Host Session' },
                  { id: 'join', label: 'Join Session' },
                ]}
                activeId={setupMode}
                onChange={(id) => {
                  setSetupMode(id as 'host' | 'join');
                  setErrorMsg(null);
                }}
                variant="pill"
              />

              {setupMode === 'host' ? (
                <div className="connect-form-stack">
                  <label className="connect-field">
                    <span>Session Name</span>
                    <input
                      type="text"
                      className="cavia-text-input"
                      value={sessionName}
                      onChange={(e) => setSessionName(e.target.value)}
                      placeholder="Friday Lounge"
                    />
                  </label>
                  <label className="connect-field">
                    <span>Host Name</span>
                    <input
                      type="text"
                      className="cavia-text-input"
                      value={hostName}
                      onChange={(e) => setHostName(e.target.value)}
                      placeholder="Your name"
                    />
                  </label>
                  <div className="connect-field">
                    <span>Fairness Weighting (α = {alpha})</span>
                    <div className="connect-alpha-row">
                      {[0.5, 1.0, 1.5, 2.0].map((val) => (
                        <button
                          key={val}
                          type="button"
                          className={`connect-alpha-btn ${alpha === val ? 'active' : ''}`}
                          onClick={() => setAlpha(val)}
                        >
                          α {val}
                        </button>
                      ))}
                    </div>
                  </div>
                  <Button
                    variant="primary"
                    onClick={handleCreate}
                    disabled={busy}
                    icon={<Radio size={15} />}
                  >
                    {busy ? 'Starting…' : 'Start Session'}
                  </Button>
                </div>
              ) : (
                <div className="connect-form-stack">
                  <label className="connect-field">
                    <span>Join Code</span>
                    <input
                      type="text"
                      className="cavia-text-input"
                      value={joinCodeInput}
                      onChange={(e) => setJoinCodeInput(e.target.value.toUpperCase())}
                      placeholder="CAVIA-XXXXXX"
                    />
                  </label>
                  <label className="connect-field">
                    <span>Your Display Name</span>
                    <input
                      type="text"
                      className="cavia-text-input"
                      value={guestName}
                      onChange={(e) => setGuestName(e.target.value)}
                      placeholder="Guest"
                    />
                  </label>
                  <Button
                    variant="primary"
                    onClick={handleJoin}
                    disabled={busy || !joinCodeInput.trim()}
                    icon={<Users size={15} />}
                  >
                    {busy ? 'Joining…' : 'Join Session'}
                  </Button>
                </div>
              )}
            </div>
          ) : (
            <div className="connect-active-stack">
              <div className="connect-tabs-row">
                <Tabs
                  items={[
                    { id: 'queue', label: `Sequence (${session.upcomingSequence.length})` },
                    { id: 'contribute', label: 'Add Music' },
                    { id: 'contributors', label: `Weights (${session.weights.length})` },
                  ]}
                  activeId={tab}
                  onChange={(id) => {
                    setTab(id as ConnectTab);
                    setErrorMsg(null);
                  }}
                  variant="pill"
                />
              </div>

              {tab === 'queue' && (
                <div className="connect-queue-tab">
                  {session.pendingPreload.length > 0 && (
                    <div className="connect-pending-banner">
                      <div className="connect-pending-head">
                        <Loader2 size={13} className="spin-accent text-vermilion" />
                        <strong>
                          Pending Verification ({session.pendingPreload.length})
                        </strong>
                      </div>
                      <p>
                        Late contributions finish downloading &amp; verifying before splicing fairly into playback.
                      </p>
                      <ul className="connect-mini-list">
                        {session.pendingPreload.map((entry) => (
                          <li key={entry.queueEntryId}>
                            <span>{entry.track.title}</span>
                            <Badge variant="neutral" size="sm">
                              {entry.contributorName}
                            </Badge>
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}

                  {session.upcomingSequence.length === 0 ? (
                    <div className="queue-blank-slate">
                      <h3>Collaborative queue is empty</h3>
                      <p>Switch to Add Music to queue YouTube Music tracks with dynamic fair ordering.</p>
                      <Button size="sm" variant="outline" onClick={() => setTab('contribute')}>
                        Add Music
                      </Button>
                    </div>
                  ) : (
                    <ol className="queue-list">
                      {session.upcomingSequence.map((entry, idx) => (
                        <li key={entry.queueEntryId} className="queue-item">
                          <span className="queue-index">{idx + 1}</span>
                          <div className="queue-item-meta">
                            <strong className="queue-item-title">{entry.track.title}</strong>
                            <span className="queue-item-artist">
                              {entry.track.artist} · by {entry.contributorName}
                              {entry.priorityOverride && ' · Override'}
                              {idx === 0 && entry.committed && ' · Next Locked'}
                            </span>
                          </div>
                          {isHost && (
                            <div className="queue-reorder-group">
                              <button
                                type="button"
                                className="queue-reorder-btn"
                                disabled={idx === 0}
                                onClick={() => handleReorder(idx, idx - 1)}
                                aria-label={`Move ${entry.track.title} up`}
                              >
                                <ChevronUp size={13} />
                              </button>
                              <button
                                type="button"
                                className="queue-reorder-btn"
                                disabled={idx === session.upcomingSequence.length - 1}
                                onClick={() => handleReorder(idx, idx + 1)}
                                aria-label={`Move ${entry.track.title} down`}
                              >
                                <ChevronDown size={13} />
                              </button>
                            </div>
                          )}
                          {(isHost || entry.contributorId === session.localParticipantId) && (
                            <button
                              type="button"
                              className="queue-remove-btn"
                              onClick={() => handleRemove(entry.queueEntryId)}
                              aria-label={`Remove ${entry.track.title}`}
                            >
                              <X size={14} />
                            </button>
                          )}
                        </li>
                      ))}
                    </ol>
                  )}
                </div>
              )}

              {tab === 'contribute' && (
                <div className="connect-contribute-tab">
                  {isHost && (
                    <label className="connect-override-toggle">
                      <input
                        type="checkbox"
                        checked={priorityOverride}
                        onChange={(e) => setPriorityOverride(e.target.checked)}
                      />
                      <Crown size={13} className="text-vermilion" />
                      <span>Priority override (host intentional jump after committed next)</span>
                    </label>
                  )}

                  <form onSubmit={handleSearchYt} className="connect-search-row">
                    <SearchInput
                      value={searchQuery}
                      onChange={setSearchQuery}
                      placeholder="Search YouTube Music to add…"
                    />
                    <Button
                      type="submit"
                      variant="primary"
                      size="sm"
                      disabled={searching || isOffline || !searchQuery.trim()}
                      icon={searching ? <Loader2 size={14} className="spin-accent" /> : <Search size={14} />}
                    >
                      Search
                    </Button>
                  </form>

                  <div className="connect-playlist-row">
                    <input
                      type="text"
                      className="cavia-text-input"
                      value={playlistUrl}
                      onChange={(e) => setPlaylistUrl(e.target.value)}
                      placeholder="YouTube Music playlist URL or ID…"
                    />
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={busy || isOffline || !playlistUrl.trim()}
                      onClick={handleContributePlaylist}
                      icon={<ListPlus size={14} />}
                    >
                      Import
                    </Button>
                  </div>

                  {ytResults.length > 0 && (
                    <div className="connect-result-group">
                      <span className="queue-section-label">YOUTUBE MUSIC RESULTS</span>
                      <ul className="connect-pick-list">
                        {ytResults.map((item) => (
                          <li key={item.videoId} className="connect-pick-row">
                            <div className="queue-item-meta">
                              <strong className="queue-item-title">{item.title}</strong>
                              <span className="queue-item-artist">
                                {item.artist} {item.album ? `· ${item.album}` : ''}
                              </span>
                            </div>
                            <Button
                              size="sm"
                              variant="outline"
                              disabled={busy}
                              onClick={() => handleContributeYt(item)}
                              icon={<Plus size={13} />}
                            >
                              Add
                            </Button>
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}

                  {isHost && localTracks.length > 0 && (
                    <div className="connect-result-group">
                      <span className="queue-section-label">HOST LIBRARY QUICK ADD</span>
                      <ul className="connect-pick-list">
                        {localTracks.slice(0, 6).map((t) => (
                          <li key={t.id} className="connect-pick-row">
                            <div className="queue-item-meta">
                              <strong className="queue-item-title">{t.title}</strong>
                              <span className="queue-item-artist">{t.artist}</span>
                            </div>
                            <Button
                              size="sm"
                              variant="ghost"
                              disabled={busy}
                              onClick={() => handleContributeLocal(t)}
                              icon={<Plus size={13} />}
                            >
                              Add
                            </Button>
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}
                </div>
              )}

              {tab === 'contributors' && (
                <div className="connect-contributors-tab">
                  {isHost && (
                    <div className="connect-alpha-card">
                      <span>Fairness strength (α = {session.fairnessAlpha})</span>
                      <div className="connect-alpha-row">
                        {[0.5, 1.0, 1.5, 2.0].map((val) => (
                          <button
                            key={val}
                            type="button"
                            className={`connect-alpha-btn ${
                              Math.abs(session.fairnessAlpha - val) < 0.05 ? 'active' : ''
                            }`}
                            onClick={() => handleAlphaChange(val)}
                          >
                            {val}
                          </button>
                        ))}
                      </div>
                    </div>
                  )}

                  <ul className="connect-weight-list">
                    {session.weights.map((p) => (
                      <li key={p.participantId} className="connect-weight-row">
                        <div>
                          <div className="connect-weight-name">
                            <strong>{p.displayName}</strong>
                            <Badge variant={p.role === 'host' ? 'vermilion' : 'neutral'} size="sm">
                              {p.role.toUpperCase()}
                            </Badge>
                          </div>
                          <span className="connect-weight-sub">
                            {p.pendingSongs} pending · {p.totalPlayed} played
                            {p.penaltyCredits > 0 ? ` · ${p.penaltyCredits.toFixed(1)} cooldown` : ''}
                          </span>
                        </div>
                        <div className="connect-weight-score">
                          <strong>{(p.weight * 100).toFixed(0)}%</strong>
                          <span>weight</span>
                        </div>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          )}
        </div>

        {session && (
          <div className="drawer-footer connect-footer-actions">
            <Button
              size="sm"
              variant="outline"
              disabled={busy || session.unsyncedEventCount === 0}
              onClick={handleSync}
              icon={<RefreshCw size={13} />}
            >
              Sync ({session.unsyncedEventCount})
            </Button>
            <Button
              size="sm"
              variant="vermilion-subtle"
              disabled={busy}
              onClick={handleEndSession}
              icon={<Trash2 size={13} />}
            >
              End Session
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
