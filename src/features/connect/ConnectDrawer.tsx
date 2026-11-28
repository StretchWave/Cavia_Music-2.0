import { useEffect, useId, useRef, useState } from 'react';
import {
  Bookmark,
  Check,
  ChevronDown,
  ChevronUp,
  Clock,
  Copy,
  Crown,
  Eye,
  EyeOff,
  ListPlus,
  Loader2,
  Plus,
  Radio,
  RefreshCw,
  Search,
  ShieldAlert,
  ShieldBan,
  Trash2,
  UserCheck,
  UserPlus,
  UserX,
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
  connectGetAccountHub,
  connectHostReorder,
  connectJoinApprovedSession,
  connectJoinSession,
  connectLookupHost,
  connectRemoveEntry,
  connectRemoveSavedHost,
  connectRespondRequest,
  connectSaveHost,
  connectSendJoinRequest,
  connectSetAlpha,
  connectSyncNow,
  connectToSavedHost,
  connectUnignoreGuest,
  connectUpdateAccountSettings,
} from '../../lib/connect';
import type {
  ConnectAccountHubSnapshot,
  ConnectSessionSnapshot,
  HostLookupResult,
  Track,
} from '../../lib/types';

interface ConnectDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  session: ConnectSessionSnapshot | null;
  hub: ConnectAccountHubSnapshot | null;
  isOffline: boolean;
  localTracks: Track[];
  onSessionChanged: (snapshot: ConnectSessionSnapshot | null) => void;
  onHubChanged: (hub: ConnectAccountHubSnapshot | null) => void;
  onNotify?: (kind: 'success' | 'error' | 'info', message: string) => void;
}

type ConnectTab = 'hosts' | 'requests' | 'queue' | 'contribute' | 'contributors';

function formatRelativeOrDate(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString(undefined, {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  });
}

function formatRemainingUntil(iso: string): string {
  const ms = new Date(iso).getTime() - Date.now();
  if (!Number.isFinite(ms) || ms <= 0) return 'Expired';
  const hours = Math.floor(ms / 3_600_000);
  const mins = Math.floor((ms % 3_600_000) / 60_000);
  if (hours > 0) return `${hours}h ${mins}m left`;
  return `${Math.max(1, mins)}m left`;
}

function renderAvailabilityLabel(status: string): { label: string; variant: 'ready' | 'warn' | 'vermilion' | 'neutral' } {
  switch (status) {
    case 'accepting_requests':
      return { label: 'Accepting requests', variant: 'ready' };
    case 'connected':
      return { label: 'Connected', variant: 'ready' };
    case 'pending':
      return { label: 'Pending approval', variant: 'vermilion' };
    case 'requests_disabled':
      return { label: 'Hosting · Closed', variant: 'warn' };
    case 'rejected':
      return { label: 'Declined', variant: 'neutral' };
    case 'unavailable':
      return { label: 'Unavailable', variant: 'neutral' };
    default:
      return { label: 'Offline', variant: 'neutral' };
  }
}

export function ConnectDrawer({
  isOpen,
  onClose,
  session,
  hub,
  isOffline,
  localTracks,
  onSessionChanged,
  onHubChanged,
  onNotify,
}: ConnectDrawerProps) {
  const [tab, setTab] = useState<ConnectTab>(session ? 'queue' : 'hosts');
  const [setupMode, setSetupMode] = useState<'host' | 'join'>('host');
  const [sessionName, setSessionName] = useState('Cavia Listening Room');
  const [hostName, setHostName] = useState(hub?.profile.displayName || 'Host');
  const [joinCodeInput, setJoinCodeInput] = useState('');
  const [guestName, setGuestName] = useState(hub?.profile.displayName || 'Guest');
  const [alpha, setAlpha] = useState(1.0);
  const [busy, setBusy] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [copiedCode, setCopiedCode] = useState(false);
  const [copiedPermanentId, setCopiedPermanentId] = useState(false);

  // Permanent ID lookup & saved hosts
  const [lookupInput, setLookupInput] = useState('');
  const [lookupResult, setLookupResult] = useState<HostLookupResult | null>(null);
  const [lookupSearched, setLookupSearched] = useState(false);

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
    connectGetAccountHub()
      .then((nextHub) => {
        if (nextHub) {
          onHubChanged(nextHub);
          if (nextHub.profile.authenticated && nextHub.profile.displayName) {
            setHostName(nextHub.profile.displayName);
            setGuestName(nextHub.profile.displayName);
          }
        }
      })
      .catch(console.error);
  }, [isOpen, onHubChanged]);

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

  const isHost = session?.localRole === 'host' || !session;
  const profile = hub?.profile ?? null;
  const isAuthenticated = Boolean(profile?.authenticated);
  const pendingRequests = (hub?.incomingRequests ?? []).filter((r) => r.status === 'pending');

  const refreshHub = async () => {
    const nextHub = await connectGetAccountHub();
    if (nextHub) onHubChanged(nextHub);
  };

  const handleCreate = async () => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const snap = await connectCreateSession(
        sessionName,
        profile?.displayName || hostName,
        alpha
      );
      onSessionChanged(snap);
      await refreshHub();
      setTab('queue');
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
      const snap = await connectJoinSession(
        joinCodeInput,
        profile?.displayName || guestName
      );
      onSessionChanged(snap);
      setTab('queue');
      onNotify?.('success', `Joined ${snap?.name ?? 'Cavia Connect'}`);
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleToggleAcceptRequests = async (next: boolean) => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const nextHub = await connectUpdateAccountSettings({ acceptJoinRequests: next });
      if (nextHub) onHubChanged(nextHub);
      onNotify?.(
        'info',
        next ? 'Accepting new connection requests.' : 'New connection requests disabled; current guests remain connected.'
      );
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleToggleShowConnectId = async (next: boolean) => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const nextHub = await connectUpdateAccountSettings({ showConnectId: next });
      if (nextHub) onHubChanged(nextHub);
      onNotify?.(
        'info',
        next ? 'Connect ID enabled in Now Playing and Bluetooth metadata.' : 'Connect ID hidden from Now Playing metadata.'
      );
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleLookupHost = async (e?: React.FormEvent) => {
    e?.preventDefault();
    const q = lookupInput.trim().toUpperCase();
    if (!q) return;
    setBusy(true);
    setErrorMsg(null);
    try {
      const found = await connectLookupHost(q);
      setLookupResult(found);
      setLookupSearched(true);
    } catch (err) {
      setLookupResult(null);
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleSaveHost = async (hostAccountId: string) => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const nextHub = await connectSaveHost(hostAccountId);
      if (nextHub) onHubChanged(nextHub);
      onNotify?.('success', 'Host saved to your Saved Hosts.');
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRemoveSavedHost = async (hostAccountId: string) => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const nextHub = await connectRemoveSavedHost(hostAccountId);
      if (nextHub) onHubChanged(nextHub);
      onNotify?.('info', 'Removed saved host.');
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleJoinApprovedSession = async (sessionId: string) => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const joined = await connectJoinApprovedSession(sessionId);
      if (joined) {
        onSessionChanged(joined);
        setTab('queue');
        onNotify?.('success', `Connected to "${joined.name}"`);
      }
      await refreshHub();
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleSendJoinRequest = async (hostAccountId: string) => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const res = await connectSendJoinRequest(hostAccountId);
      if (res) {
        if (res.outcome === 'connected' && res.sessionId) {
          const joined = await connectJoinApprovedSession(res.sessionId);
          if (joined) {
            onSessionChanged(joined);
            setTab('queue');
            onNotify?.('success', `Connected to "${joined.name}"`);
          }
        } else {
          onNotify?.(res.outcome === 'pending' ? 'success' : 'info', res.message);
        }
        if (lookupResult?.hostAccountId === hostAccountId) {
          const updated = await connectLookupHost(lookupResult.connectId).catch(() => null);
          if (updated) setLookupResult(updated);
        }
        await refreshHub();
      }
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleConnectSavedHost = async (hostAccountId: string) => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const res = await connectToSavedHost(hostAccountId);
      if (res) {
        if (res.outcome === 'connected' && res.sessionId) {
          const joined = await connectJoinApprovedSession(res.sessionId);
          if (joined) {
            onSessionChanged(joined);
            setTab('queue');
            onNotify?.('success', `Connected to "${joined.name}"`);
          }
        } else {
          onNotify?.(res.outcome === 'pending' ? 'success' : 'info', res.message);
        }
        await refreshHub();
      }
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRespondRequest = async (
    requestId: string,
    action: 'accept' | 'decline' | 'ignore_1d'
  ) => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const res = await connectRespondRequest(requestId, action);
      if (res) {
        const label =
          action === 'accept'
            ? `Accepted ${res.requesterDisplayName}`
            : action === 'decline'
              ? `Declined ${res.requesterDisplayName}`
              : `Ignored ${res.requesterDisplayName} for 24 hours`;
        onNotify?.('info', label);
      }
      await refreshHub();
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleUnignore = async (ignoredAccountId: string) => {
    setBusy(true);
    setErrorMsg(null);
    try {
      const nextHub = await connectUnignoreGuest(ignoredAccountId);
      if (nextHub) onHubChanged(nextHub);
      onNotify?.('info', 'Account unignored.');
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleCopy = async (text: string, which: 'session' | 'permanent') => {
    try {
      await navigator.clipboard.writeText(text);
      if (which === 'session') {
        setCopiedCode(true);
        setTimeout(() => setCopiedCode(false), 1800);
      } else {
        setCopiedPermanentId(true);
        setTimeout(() => setCopiedPermanentId(false), 1800);
      }
    } catch {
      onNotify?.('info', text);
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
      await refreshHub();
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
      await refreshHub();
      onNotify?.('info', 'Closed Cavia Connect hosting session.');
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const savedHostIds = new Set((hub?.savedHosts ?? []).map((h) => h.hostAccountId));

  const tabItems = [
    { id: 'hosts', label: `Hosts (${hub?.savedHosts.length ?? 0})` },
    {
      id: 'requests',
      label: pendingRequests.length > 0 ? `Requests (${pendingRequests.length})` : 'Requests',
    },
    ...(session
      ? [
          { id: 'queue', label: `Sequence (${session.upcomingSequence.length})` },
          { id: 'contribute', label: 'Add Music' },
          { id: 'contributors', label: `Weights (${session.weights.length})` },
        ]
      : []),
  ];

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
                  {session.localRole.toUpperCase()}
                </Badge>
              )}
            </div>
            <div className="connect-subbar">
              {profile?.connectId && (
                <button
                  type="button"
                  className="connect-code-pill"
                  onClick={() => handleCopy(profile.connectId!, 'permanent')}
                  title="Permanent Cavia ID (never changes across sessions or devices)"
                >
                  <span>ID: {profile.connectId}</span>
                  {copiedPermanentId ? <Check size={12} /> : <Copy size={12} />}
                </button>
              )}
              {session && (
                <button
                  type="button"
                  className="connect-session-pill"
                  onClick={() => handleCopy(session.joinCode, 'session')}
                  title="Active session join code"
                >
                  <span>Session: {session.joinCode}</span>
                  {copiedCode ? <Check size={11} /> : <Copy size={11} />}
                </button>
              )}
              {isOffline && (
                <Badge variant="warn" size="sm">
                  <WifiOff size={11} /> Offline
                </Badge>
              )}
            </div>
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

          {/* Account Authentication & Three Independent Connect Controls */}
          {!isAuthenticated ? (
            <div className="connect-auth-notice">
              <ShieldAlert size={15} className="text-vermilion" />
              <div>
                <strong>Account sign-in required for Permanent Cavia ID &amp; Requests</strong>
                <p>
                  Sign in under Settings &rarr; Profile to receive your unique 6-character Connect ID, approve incoming requests, ignore accounts, and save hosts. Local fair-queue sessions can still be started below.
                </p>
              </div>
            </div>
          ) : (
            <div className="connect-controls-card">
              <div className="connect-control-row">
                <div>
                  <strong>Hosting Session</strong>
                  <span>{session ? `Active (${session.joinCode})` : 'Inactive'}</span>
                </div>
                {session ? (
                  <Button
                    size="sm"
                    variant="vermilion-subtle"
                    disabled={busy}
                    onClick={handleEndSession}
                  >
                    End Session
                  </Button>
                ) : (
                  <Badge variant="neutral" size="sm">OFF</Badge>
                )}
              </div>

              <label className="connect-control-row connect-toggle-row">
                <div>
                  <strong>Accept Join Requests</strong>
                  <span>
                    Allow new guests &amp; saved hosts to request access. Existing guests stay connected when off.
                  </span>
                </div>
                <input
                  type="checkbox"
                  className="cavia-toggle"
                  checked={Boolean(profile?.acceptJoinRequests)}
                  disabled={busy}
                  onChange={(e) => handleToggleAcceptRequests(e.target.checked)}
                />
              </label>

              <label className="connect-control-row connect-toggle-row">
                <div>
                  <strong>
                    {profile?.showConnectId ? <Eye size={13} /> : <EyeOff size={13} />} Show Connect ID
                  </strong>
                  <span>
                    Include your permanent Connect ID in Windows/Android Now Playing and Bluetooth metadata while hosting.
                  </span>
                </div>
                <input
                  type="checkbox"
                  className="cavia-toggle"
                  checked={Boolean(profile?.showConnectId)}
                  disabled={busy}
                  onChange={(e) => handleToggleShowConnectId(e.target.checked)}
                />
              </label>
            </div>
          )}

          <div className="connect-tabs-row">
            <Tabs
              items={tabItems}
              activeId={tab}
              onChange={(id) => {
                setTab(id as ConnectTab);
                setErrorMsg(null);
              }}
              variant="pill"
            />
          </div>

          {tab === 'hosts' && (
            <div className="connect-hosts-tab">
              {isAuthenticated && (
                <div className="connect-lookup-card">
                  <span className="queue-section-label">FIND HOST BY PERMANENT ID</span>
                  <form onSubmit={handleLookupHost} className="connect-search-row">
                    <input
                      type="text"
                      maxLength={6}
                      className="cavia-text-input connect-id-input"
                      value={lookupInput}
                      onChange={(e) => setLookupInput(e.target.value.toUpperCase())}
                      placeholder="6-char Connect ID (e.g. H4K7MN)"
                      aria-label="Host permanent Connect ID"
                    />
                    <Button
                      type="submit"
                      size="sm"
                      variant="primary"
                      disabled={busy || lookupInput.trim().length < 6}
                      icon={<Search size={14} />}
                    >
                      Lookup
                    </Button>
                  </form>

                  {lookupSearched && !lookupResult && (
                    <p className="connect-empty-hint">
                      No account found with that permanent Connect ID.
                    </p>
                  )}

                  {lookupResult && (
                    <div className="connect-lookup-result">
                      <div className="connect-identity-row">
                        {lookupResult.avatarUrl ? (
                          <img
                            src={lookupResult.avatarUrl}
                            alt=""
                            className="connect-avatar"
                          />
                        ) : (
                          <div className="connect-avatar fallback">
                            {lookupResult.displayName.slice(0, 1).toUpperCase()}
                          </div>
                        )}
                        <div className="queue-item-meta">
                          <strong className="queue-item-title">{lookupResult.displayName}</strong>
                          <span className="queue-item-artist">
                            ID: {lookupResult.connectId}
                          </span>
                        </div>
                        <Badge
                          variant={renderAvailabilityLabel(lookupResult.availabilityStatus).variant}
                          size="sm"
                        >
                          {renderAvailabilityLabel(lookupResult.availabilityStatus).label}
                        </Badge>
                      </div>

                      <div className="connect-lookup-actions">
                        {lookupResult.availabilityStatus !== 'self' && (
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={busy || savedHostIds.has(lookupResult.hostAccountId)}
                            onClick={() => handleSaveHost(lookupResult.hostAccountId)}
                            icon={<Bookmark size={13} />}
                          >
                            {savedHostIds.has(lookupResult.hostAccountId) ? 'Saved' : 'Save Host'}
                          </Button>
                        )}
                        <Button
                          size="sm"
                          variant="primary"
                          disabled={
                            busy ||
                            (!lookupResult.canRequest &&
                              !(lookupResult.alreadyApproved && lookupResult.hosting))
                          }
                          onClick={() => handleSendJoinRequest(lookupResult.hostAccountId)}
                          icon={<UserPlus size={13} />}
                        >
                          {lookupResult.alreadyApproved
                            ? 'Join Session'
                            : lookupResult.pendingRequest
                              ? 'Request Pending'
                              : 'Send Join Request'}
                        </Button>
                      </div>
                    </div>
                  )}
                </div>
              )}

              {isAuthenticated && (
                <div className="connect-saved-section">
                  <span className="queue-section-label">
                    SAVED HOSTS ({hub?.savedHosts.length ?? 0})
                  </span>
                  {(hub?.savedHosts.length ?? 0) === 0 ? (
                    <p className="connect-empty-hint">
                      Save hosts when looking up their Connect ID so you can request to join without re-entering codes.
                    </p>
                  ) : (
                    <ul className="connect-pick-list">
                      {(hub?.savedHosts ?? []).map((host) => {
                        const avail = renderAvailabilityLabel(host.availabilityStatus);
                        const canConnect = host.availabilityStatus === 'accepting_requests';
                        return (
                          <li key={host.hostAccountId} className="connect-saved-host-card">
                            <div className="connect-identity-row">
                              {host.hostAvatarUrl ? (
                                <img src={host.hostAvatarUrl} alt="" className="connect-avatar" />
                              ) : (
                                <div className="connect-avatar fallback">
                                  {host.hostDisplayName.slice(0, 1).toUpperCase()}
                                </div>
                              )}
                              <div className="queue-item-meta">
                                <strong className="queue-item-title">
                                  {host.hostDisplayName}
                                </strong>
                                <span className="queue-item-artist">
                                  ID: {host.hostConnectId}
                                </span>
                              </div>
                              <Badge variant={avail.variant} size="sm">
                                {avail.label}
                              </Badge>
                            </div>
                            <div className="connect-lookup-actions">
                              <Button
                                size="sm"
                                variant="primary"
                                disabled={
                                  busy ||
                                  (!canConnect &&
                                    host.availabilityStatus !== 'pending' &&
                                    host.availabilityStatus !== 'connected')
                                }
                                onClick={() => handleConnectSavedHost(host.hostAccountId)}
                              >
                                {host.availabilityStatus === 'pending'
                                  ? 'Pending'
                                  : host.availabilityStatus === 'connected'
                                    ? 'Join Session'
                                    : 'Connect'}
                              </Button>
                              <Button
                                size="sm"
                                variant="ghost"
                                disabled={busy}
                                onClick={() => handleRemoveSavedHost(host.hostAccountId)}
                                icon={<Trash2 size={13} />}
                                aria-label={`Remove saved host ${host.hostDisplayName}`}
                              >
                                Remove
                              </Button>
                            </div>
                          </li>
                        );
                      })}
                    </ul>
                  )}
                </div>
              )}

              {!session && (
                <div className="connect-setup-stack">
                  <Tabs
                    items={[
                      { id: 'host', label: 'Start Host Session' },
                      { id: 'join', label: 'Join Local Code' },
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
                      {!isAuthenticated && (
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
                      )}
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
                        {busy ? 'Starting…' : 'Start Hosting Session'}
                      </Button>
                    </div>
                  ) : (
                    <div className="connect-form-stack">
                      <label className="connect-field">
                        <span>Session Join Code</span>
                        <input
                          type="text"
                          className="cavia-text-input"
                          value={joinCodeInput}
                          onChange={(e) => setJoinCodeInput(e.target.value.toUpperCase())}
                          placeholder="CAVIA-XXXXXX"
                        />
                      </label>
                      {!isAuthenticated && (
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
                      )}
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
              )}
            </div>
          )}

          {tab === 'requests' && (
            <div className="connect-requests-tab">
              <div className="connect-req-section">
                <div className="connect-section-head">
                  <span className="queue-section-label">
                    INCOMING HOST REQUESTS ({hub?.incomingRequests.length ?? 0})
                  </span>
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={busy}
                    onClick={refreshHub}
                    icon={<RefreshCw size={12} />}
                  >
                    Refresh
                  </Button>
                </div>

                {!isAuthenticated ? (
                  <p className="connect-empty-hint">Sign in to receive and manage connection requests.</p>
                ) : (hub?.incomingRequests.length ?? 0) === 0 ? (
                  <p className="connect-empty-hint">No connection requests received.</p>
                ) : (
                  <ul className="connect-pick-list">
                    {(hub?.incomingRequests ?? []).map((req) => (
                      <li key={req.requestId} className="connect-request-card">
                        <div className="connect-identity-row">
                          {req.requesterAvatarUrl ? (
                            <img src={req.requesterAvatarUrl} alt="" className="connect-avatar" />
                          ) : (
                            <div className="connect-avatar fallback">
                              {req.requesterDisplayName.slice(0, 1).toUpperCase()}
                            </div>
                          )}
                          <div className="queue-item-meta">
                            <strong className="queue-item-title">{req.requesterDisplayName}</strong>
                            <span className="queue-item-artist">
                              ID: {req.requesterConnectId} · {formatRelativeOrDate(req.createdAt)}
                            </span>
                          </div>
                          <Badge
                            variant={
                              req.status === 'pending'
                                ? 'vermilion'
                                : req.status === 'accepted'
                                  ? 'ready'
                                  : 'neutral'
                            }
                            size="sm"
                          >
                            {req.status.toUpperCase()}
                          </Badge>
                        </div>

                        {req.status === 'pending' && (
                          <div className="connect-request-actions">
                            <Button
                              size="sm"
                              variant="primary"
                              disabled={busy}
                              onClick={() => handleRespondRequest(req.requestId, 'accept')}
                              icon={<UserCheck size={13} />}
                            >
                              Accept
                            </Button>
                            <Button
                              size="sm"
                              variant="outline"
                              disabled={busy}
                              onClick={() => handleRespondRequest(req.requestId, 'decline')}
                              icon={<UserX size={13} />}
                            >
                              Decline
                            </Button>
                            <Button
                              size="sm"
                              variant="vermilion-subtle"
                              disabled={busy}
                              onClick={() => handleRespondRequest(req.requestId, 'ignore_1d')}
                              icon={<ShieldBan size={13} />}
                            >
                              Ignore 1d
                            </Button>
                          </div>
                        )}
                      </li>
                    ))}
                  </ul>
                )}
              </div>

              {isAuthenticated && (hub?.outboundRequests.length ?? 0) > 0 && (
                <div className="connect-req-section">
                  <span className="queue-section-label">YOUR SENT REQUESTS</span>
                  <ul className="connect-pick-list">
                    {(hub?.outboundRequests ?? []).map((req) => (
                      <li key={req.requestId} className="connect-pick-row">
                        <div className="queue-item-meta">
                          <strong className="queue-item-title">{req.hostDisplayName}</strong>
                          <span className="queue-item-artist">
                            ID: {req.hostConnectId} · {formatRelativeOrDate(req.createdAt)}
                          </span>
                        </div>
                        <div className="connect-row-actions">
                          <Badge
                            variant={
                              req.status === 'accepted'
                                ? 'ready'
                                : req.status === 'pending'
                                  ? 'vermilion'
                                  : 'neutral'
                            }
                            size="sm"
                          >
                            {req.status.toUpperCase()}
                          </Badge>
                          {req.status === 'accepted' && req.sessionId && (
                            <Button
                              size="sm"
                              variant="primary"
                              disabled={busy}
                              onClick={() => handleJoinApprovedSession(req.sessionId)}
                            >
                              Join
                            </Button>
                          )}
                        </div>
                      </li>
                    ))}
                  </ul>
                </div>
              )}

              {isAuthenticated && (
                <div className="connect-req-section">
                  <span className="queue-section-label">
                    IGNORED ACCOUNTS ({hub?.ignoredAccounts.length ?? 0})
                  </span>
                  {(hub?.ignoredAccounts.length ?? 0) === 0 ? (
                    <p className="connect-empty-hint">
                      No accounts are currently ignored for 24 hours.
                    </p>
                  ) : (
                    <ul className="connect-pick-list">
                      {(hub?.ignoredAccounts ?? []).map((ign) => (
                        <li key={ign.ignoredAccountId} className="connect-pick-row">
                          <div className="queue-item-meta">
                            <strong className="queue-item-title">{ign.displayName}</strong>
                            <span className="queue-item-artist">
                              ID: {ign.connectId} · <Clock size={11} /> {formatRemainingUntil(ign.expiresAt)}
                            </span>
                          </div>
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={busy}
                            onClick={() => handleUnignore(ign.ignoredAccountId)}
                          >
                            Unignore
                          </Button>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              )}
            </div>
          )}

          {session && tab === 'queue' && (
            <div className="connect-queue-tab">
              {session.pendingPreload.length > 0 && (
                <div className="connect-pending-banner">
                  <div className="connect-pending-head">
                    <Loader2 size={13} className="spin-accent text-vermilion" />
                    <strong>Pending Verification ({session.pendingPreload.length})</strong>
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
                      {session.localRole === 'host' && (
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
                      {(session.localRole === 'host' ||
                        entry.contributorId === session.localParticipantId) && (
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

          {session && tab === 'contribute' && (
            <div className="connect-contribute-tab">
              {session.localRole === 'host' && (
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

              {session.localRole === 'host' && localTracks.length > 0 && (
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

          {session && tab === 'contributors' && (
            <div className="connect-contributors-tab">
              {session.localRole === 'host' && (
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
