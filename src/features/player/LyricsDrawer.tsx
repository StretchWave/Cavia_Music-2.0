import { useState, useEffect } from 'react';
import {
  ExternalLink,
  Mic2,
  Radio,
  Sparkles,
  Volume2,
  X,
} from 'lucide-react';
import { IconButton } from '../../components/IconButton';
import { Badge } from '../../components/Badge';
import { Tabs } from '../../components/Tabs';
import type { Track } from '../../lib/types';

interface LyricsDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  currentTrack: Track | null;
}

export function LyricsDrawer({ isOpen, onClose, currentTrack }: LyricsDrawerProps) {
  const [mode, setMode] = useState<'synced' | 'plain'>('synced');

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape' && isOpen) {
        onClose();
      }
    }
    if (isOpen) {
      window.addEventListener('keydown', handleKeyDown);
    }
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  return (
    <aside className="cavia-drawer-backdrop" onClick={onClose} role="dialog" aria-modal="true" aria-label="Lyrics archive">
      <div className="cavia-drawer-panel lyrics-panel" onClick={(e) => e.stopPropagation()}>
        <div className="sheet-drag-handle" aria-hidden="true" />
        {/* Header */}
        <div className="drawer-header">
          <div>
            <div className="drawer-eyebrow">
              <Sparkles size={12} />
              <span>LYRICS ARCHIVE</span>
            </div>
            <div className="drawer-title-row">
              <h2 className="drawer-title">
                {currentTrack ? currentTrack.title : 'Lyrics View'}
              </h2>
            </div>
            <span className="lyrics-track-artist">
              {currentTrack?.artist ? `${currentTrack.artist} — ${currentTrack.album}` : 'Standby'}
            </span>
          </div>

          <div className="drawer-header-right">
            <Tabs
              items={[
                { id: 'synced', label: 'Synced' },
                { id: 'plain', label: 'Plain' },
              ]}
              activeId={mode}
              onChange={setMode}
              variant="pill"
            />
            <IconButton
              label="Close lyrics"
              icon={<X size={18} />}
              variant="ghost"
              onClick={onClose}
            />
          </div>
        </div>

        {/* Body */}
        <div className="drawer-body lyrics-body">
          {currentTrack ? (
            <div className="lyrics-container">
              <div className="lyrics-notice-card">
                <Radio size={16} />
                <div className="lyrics-notice-text">
                  <strong>LRCLIB Community Integration</strong>
                  <p>
                    Synchronized lines will stream via the open LRCLIB adapter once local audio is loaded. In accordance with product rules, no fake lyrics are shown.
                  </p>
                </div>
                <Badge variant="outline" size="sm">
                  MILESTONE 4
                </Badge>
              </div>

              {/* Sample of the real typography container that will display lines */}
              <div className="lyrics-typography-stage">
                <p className="lyrics-line inactive">Waiting for audio playback synchronizer...</p>
                <p className="lyrics-line active-line">
                  {currentTrack.title}
                </p>
                <p className="lyrics-line inactive">
                  {currentTrack.artist}
                </p>
              </div>
            </div>
          ) : (
            <div className="lyrics-empty-state">
              <div className="lyrics-empty-icon">
                <Mic2 size={32} />
              </div>
              <h3>No track in playback</h3>
              <p>
                When you play a track, synchronized or plain lyrics will automatically be matched using embedded metadata and the LRCLIB provider.
              </p>

              <div className="lyrics-feature-list">
                <div className="lyrics-feature-item">
                  <Volume2 size={16} />
                  <span>Real-time line tracking synchronized with audio playback</span>
                </div>
                <div className="lyrics-feature-item">
                  <ExternalLink size={16} />
                  <span>LRCLIB community open database for multi-language lyrics</span>
                </div>
                <div className="lyrics-feature-item">
                  <Sparkles size={16} />
                  <span>Supports embedded ID3 USLT and SYLT metadata tags</span>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="drawer-footer">
          <span className="drawer-footer-text">
            Lyrics caching complies with provider agreements and respects offline privacy.
          </span>
        </div>
      </div>
    </aside>
  );
}

