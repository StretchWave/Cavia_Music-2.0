import { useState } from 'react';
import {
  AudioWaveform,
  CheckCircle2,
  Database,
  HardDrive,
  Info,
  Layers,
  Lock,
  Music2,
  Palette,
  Radio,
  RadioTower,
  RefreshCw,
  ShieldCheck,
  Sliders,
  Sparkles,
  Volume2,
} from 'lucide-react';
import { Tabs } from '../../components/Tabs';
import { Badge } from '../../components/Badge';
import { Button } from '../../components/Button';
import { Modal } from '../../components/Modal';
import type { PlatformHealth } from '../../lib/platform';
import type { ProviderInfo } from '../../lib/types';

interface SettingsViewProps {
  health: PlatformHealth | null;
  density: 'standard' | 'compact';
  onDensityChange: (density: 'standard' | 'compact') => void;
  showKanji: boolean;
  onShowKanjiChange: (show: boolean) => void;
}

type SettingsSection = 'appearance' | 'audio' | 'library' | 'providers' | 'about';

export function SettingsView({
  health,
  density,
  onDensityChange,
  showKanji,
  onShowKanjiChange,
}: SettingsViewProps) {
  const [activeTab, setActiveTab] = useState<SettingsSection>('appearance');
  const [accentTheme, setAccentTheme] = useState<'vermilion' | 'amber' | 'ivory'>('vermilion');
  const [selectedProvider, setSelectedProvider] = useState<ProviderInfo | null>(null);

  const providers: ProviderInfo[] = [
    {
      id: 'local_storage',
      name: 'Local Audio Engine',
      category: 'local',
      description: 'Reads local audio files (FLAC, MP3, WAV, AAC) directly from your device storage with zero network traffic.',
      status: 'operational',
      capabilities: {
        catalogueSearch: true,
        metadataLookup: true,
        playback: true,
        caching: true,
        lyrics: true,
      },
      notes: 'Active by default. Staged safely through platform-native file dialogs.',
    },
    {
      id: 'lrclib',
      name: 'LRCLIB Lyrics Database',
      category: 'lyrics',
      description: 'Open-source community-driven synchronized lyrics service. Non-commercial and legally authorized.',
      status: 'planned',
      capabilities: {
        catalogueSearch: false,
        metadataLookup: false,
        playback: false,
        caching: true,
        lyrics: true,
      },
      notes: 'Planned for Milestone 4. No account or API secret required.',
    },
    {
      id: 'spotify',
      name: 'Spotify PKCE',
      category: 'metadata',
      description: 'Authorized OAuth 2.0 PKCE integration for playlist import and catalogue metadata lookups.',
      status: 'unconfigured',
      capabilities: {
        catalogueSearch: true,
        metadataLookup: true,
        playback: false,
        caching: false,
        lyrics: false,
      },
      notes: 'Metadata only. Does not scrape or bypass DRM. Credentials stored in platform secure storage.',
    },
    {
      id: 'musicbrainz',
      name: 'MusicBrainz',
      category: 'metadata',
      description: 'Open music encyclopedia for enriched discography and release credit resolution.',
      status: 'planned',
      capabilities: {
        catalogueSearch: true,
        metadataLookup: true,
        playback: false,
        caching: true,
        lyrics: false,
      },
      notes: 'Community-maintained open database integration.',
    },
  ];

  return (
    <section className="cavia-subpage settings-page">
      <div className="subpage-header">
        <div>
          <div className="cavia-eyebrow">
            <span className="eyebrow-accent-line" />
            <span>CONFIGURATION & ARCHITECTURE</span>
          </div>
          <h1 className="subpage-title">
            Settings &<br />
            <em>system.</em>
          </h1>
          <p className="subpage-subtitle">
            Configure UI nuance, inspect audio boundaries, and audit local storage status.
          </p>
        </div>
      </div>

      {/* Tabs */}
      <div className="settings-tabs-wrap">
        <Tabs
          items={[
            { id: 'appearance', label: 'Appearance', icon: <Palette size={14} /> },
            { id: 'audio', label: 'Audio Engine', icon: <Volume2 size={14} /> },
            { id: 'library', label: 'Library & SQLite', icon: <Database size={14} /> },
            { id: 'providers', label: 'Providers', icon: <Radio size={14} /> },
            { id: 'about', label: 'About & Policy', icon: <Info size={14} /> },
          ]}
          activeId={activeTab}
          onChange={setActiveTab}
          variant="segmented"
        />
      </div>

      {/* Tab: Appearance */}
      {activeTab === 'appearance' && (
        <div className="settings-content-pane">
          <div className="settings-card">
            <div className="card-head">
              <Palette size={18} className="card-icon" />
              <div>
                <h3>Accent Tint</h3>
                <p>Selected tone applied to active indicators, glints, and volume sliders.</p>
              </div>
            </div>
            <div className="accent-picker-row">
              <button
                type="button"
                className={`accent-option vermilion ${accentTheme === 'vermilion' ? 'selected' : ''}`}
                onClick={() => setAccentTheme('vermilion')}
              >
                <span className="swatch" style={{ background: '#d64732' }} />
                <span>Vermilion (Default)</span>
              </button>
              <button
                type="button"
                className={`accent-option amber ${accentTheme === 'amber' ? 'selected' : ''}`}
                onClick={() => setAccentTheme('amber')}
              >
                <span className="swatch" style={{ background: '#e09f58' }} />
                <span>Vintage Amber</span>
              </button>
              <button
                type="button"
                className={`accent-option ivory ${accentTheme === 'ivory' ? 'selected' : ''}`}
                onClick={() => setAccentTheme('ivory')}
              >
                <span className="swatch" style={{ background: '#f5f2eb' }} />
                <span>Aged Ivory</span>
              </button>
            </div>
          </div>

          <div className="settings-card">
            <div className="card-head">
              <Layers size={18} className="card-icon" />
              <div>
                <h3>Interface Density</h3>
                <p>Adjust padding, row heights, and player dock sizing for high-res displays.</p>
              </div>
            </div>
            <div className="setting-control-row">
              <Tabs
                items={[
                  { id: 'standard', label: 'Standard (Relaxed)' },
                  { id: 'compact', label: 'Compact (Studio)' },
                ]}
                activeId={density}
                onChange={onDensityChange}
                variant="pill"
              />
            </div>
          </div>

          <div className="settings-card">
            <div className="card-head">
              <Sparkles size={18} className="card-icon" />
              <div>
                <h3>Watermark Japanese Calligraphy</h3>
                <p>Display subtle background kanji watermarks (藏 / 音 / 盤) across archive pages.</p>
              </div>
            </div>
            <div className="setting-control-row">
              <button
                type="button"
                className={`cavia-toggle-btn ${showKanji ? 'active' : ''}`}
                onClick={() => onShowKanjiChange(!showKanji)}
              >
                <span className="toggle-track">
                  <span className="toggle-thumb" />
                </span>
                <span>{showKanji ? 'Visible' : 'Hidden'}</span>
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Tab: Audio Engine */}
      {activeTab === 'audio' && (
        <div className="settings-content-pane">
          <div className="cavia-banner banner-info">
            <Music2 size={18} className="banner-icon-accent" />
            <div className="banner-content">
              <strong>Playback Engine Boundary Architecture</strong>
              <p>
                All platforms share a common Cavia audio contract. Windows and macOS utilize the <code>rsmpv</code> libmpv candidate; Android utilizes Media3 / ExoPlayer; iOS utilizes AVPlayer. Controls remain safely disabled until concrete backend initialization occurs.
              </p>
            </div>
            <Badge variant="warn" size="sm">
              MILESTONE 3
            </Badge>
          </div>

          <div className="settings-grid-two">
            <div className="settings-card">
              <div className="card-head">
                <Sliders size={18} className="card-icon" />
                <div>
                  <h3>Audio Backend Status</h3>
                  <p>Target: {health?.platform ?? 'Windows x86_64'}</p>
                </div>
              </div>
              <div className="setting-status-line">
                <Badge variant={health?.audio_backend === 'ready' ? 'ready' : 'outline'} size="md">
                  {health?.audio_backend === 'ready' ? 'INTEGRATED' : 'NOT INTEGRATED (STANDBY)'}
                </Badge>
                <span className="text-muted-xs">Contract ready · No mock audio</span>
              </div>
            </div>

            <div className="settings-card">
              <div className="card-head">
                <Volume2 size={18} className="card-icon" />
                <div>
                  <h3>Output Resampling</h3>
                  <p>High-resolution bit-perfect streaming</p>
                </div>
              </div>
              <div className="setting-status-line">
                <span className="text-dim-sm">Direct WASAPI / CoreAudio (Milestone 3)</span>
                <Badge variant="neutral" size="sm">
                  STANDBY
                </Badge>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Tab: Library & SQLite */}
      {activeTab === 'library' && (
        <div className="settings-content-pane">
          <div className="settings-card">
            <div className="card-head">
              <Database size={18} className="card-icon" />
              <div>
                <h3>SQLite Database Engine</h3>
                <p>Rust-owned SQLite connection managed with compile-time SQLx migrations.</p>
              </div>
            </div>
            <div className="sqlite-stats-grid">
              <div className="sqlite-stat-box">
                <span className="box-label">CONNECTION STATUS</span>
                <strong className="box-val text-ready">
                  {health?.database_connected ? 'CONNECTED & HEALTHY' : 'DISCONNECTED'}
                </strong>
              </div>
              <div className="sqlite-stat-box">
                <span className="box-label">SCHEMA MIGRATION</span>
                <strong className="box-val">0001_initial.sql (Active)</strong>
              </div>
              <div className="sqlite-stat-box">
                <span className="box-label">SQLITE JOURNAL</span>
                <strong className="box-val">WAL Mode (Configured)</strong>
              </div>
            </div>
          </div>

          <div className="settings-card">
            <div className="card-head">
              <HardDrive size={18} className="card-icon" />
              <div>
                <h3>Local Storage & Cache Policy</h3>
                <p>Artwork thumbnails and audio caching rules.</p>
              </div>
            </div>
            <div className="setting-control-row">
              <span className="text-dim-sm">No cached remote audio. Artwork cached in app sandbox.</span>
              <Button
                variant="outline"
                size="sm"
                icon={<RefreshCw size={13} />}
                disabled
                title="Cache cleanup planned for Milestone 4"
              >
                Clear Artwork Cache
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* Tab: Providers */}
      {activeTab === 'providers' && (
        <div className="settings-content-pane">
          <div className="cavia-banner banner-info">
            <RadioTower size={18} className="banner-icon-accent" />
            <div className="banner-content">
              <strong>Legal Compliance & Authorized Streaming</strong>
              <p>
                Cavia does not scrape undocumented endpoints or bypass DRM or paywalls. Provider integrations strictly distinguish catalogue search, metadata resolution, playback permissions, and caching rights.
              </p>
            </div>
          </div>

          <div className="providers-grid">
            {providers.map((p) => (
              <div
                key={p.id}
                className="provider-card"
                onClick={() => setSelectedProvider(p)}
                tabIndex={0}
                role="button"
              >
                <div className="provider-header-row">
                  <div className="provider-name-group">
                    <strong className="provider-name">{p.name}</strong>
                    <span className="provider-cat">{p.category.toUpperCase()}</span>
                  </div>
                  <Badge
                    variant={
                      p.status === 'operational'
                        ? 'ready'
                        : p.status === 'unconfigured'
                        ? 'neutral'
                        : 'outline'
                    }
                    size="sm"
                  >
                    {p.status.toUpperCase()}
                  </Badge>
                </div>

                <p className="provider-desc">{p.description}</p>

                <div className="provider-caps-list">
                  <span className={`cap-chip ${p.capabilities.catalogueSearch ? 'active' : ''}`}>
                    Search
                  </span>
                  <span className={`cap-chip ${p.capabilities.metadataLookup ? 'active' : ''}`}>
                    Metadata
                  </span>
                  <span className={`cap-chip ${p.capabilities.playback ? 'active' : ''}`}>
                    Playback
                  </span>
                  <span className={`cap-chip ${p.capabilities.caching ? 'active' : ''}`}>
                    Caching
                  </span>
                  <span className={`cap-chip ${p.capabilities.lyrics ? 'active' : ''}`}>
                    Lyrics
                  </span>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Tab: About */}
      {activeTab === 'about' && (
        <div className="settings-content-pane">
          <div className="settings-card about-hero-card">
            <div className="about-brand-row">
              <AudioWaveform size={28} className="text-vermilion" />
              <div>
                <h2>cavia music<span>.</span></h2>
                <span className="about-edition">VERSION 2.0.0 · THE ARCHIVE RELEASE</span>
              </div>
            </div>

            <p className="about-text">
              A high-fidelity, local-first music application designed for listeners who value craft, typography, and uncompromised privacy. Built with Tauri 2, React 19, TypeScript, and Rust.
            </p>

            <div className="about-badges">
              <Badge variant="vermilion" size="sm">TAURI 2</Badge>
              <Badge variant="neutral" size="sm">SQLITE / SQLX</Badge>
              <Badge variant="neutral" size="sm">REACT 19</Badge>
              <Badge variant="neutral" size="sm">RUST CORE</Badge>
              <Badge variant="outline" size="sm">NO TELEMETRY</Badge>
            </div>
          </div>

          <div className="settings-card">
            <div className="card-head">
              <ShieldCheck size={18} className="card-icon text-ready" />
              <div>
                <h3>Architectural Principles</h3>
                <p>Non-negotiable product integrity rules</p>
              </div>
            </div>
            <ul className="principles-list">
              <li>
                <CheckCircle2 size={14} className="text-ready" />
                <span><strong>No fabricated data:</strong> Never show fake tracks, albums, or synthetic playback progress.</span>
              </li>
              <li>
                <CheckCircle2 size={14} className="text-ready" />
                <span><strong>Platform secure storage:</strong> Secrets and tokens are kept in OS keychains, never plaintext SQLite.</span>
              </li>
              <li>
                <CheckCircle2 size={14} className="text-ready" />
                <span><strong>Local audio sovereignty:</strong> Your files remain untouched and completely offline.</span>
              </li>
              <li>
                <CheckCircle2 size={14} className="text-ready" />
                <span><strong>Honest control states:</strong> Controls whose backends are pending display disabled indicators.</span>
              </li>
            </ul>
          </div>
        </div>
      )}

      {/* Provider Details Modal */}
      {selectedProvider && (
        <Modal
          isOpen={!!selectedProvider}
          onClose={() => setSelectedProvider(null)}
          title={selectedProvider.name}
          eyebrow="PROVIDER SPECIFICATION"
          size="md"
          footer={
            <Button variant="ghost" size="sm" onClick={() => setSelectedProvider(null)}>
              Dismiss
            </Button>
          }
        >
          <div className="provider-modal-body">
            <div className="provider-modal-badge-row">
              <Badge
                variant={selectedProvider.status === 'operational' ? 'ready' : 'outline'}
                size="md"
              >
                STATUS: {selectedProvider.status.toUpperCase()}
              </Badge>
              <span className="text-dim-sm">Category: {selectedProvider.category}</span>
            </div>

            <p className="provider-modal-desc">{selectedProvider.description}</p>

            <div className="provider-caps-table">
              <h4>Declared Capabilities</h4>
              <div className="caps-row">
                <span>Catalogue Search</span>
                <strong>{selectedProvider.capabilities.catalogueSearch ? 'Permitted' : 'Not Supported'}</strong>
              </div>
              <div className="caps-row">
                <span>Metadata Resolution</span>
                <strong>{selectedProvider.capabilities.metadataLookup ? 'Permitted' : 'Not Supported'}</strong>
              </div>
              <div className="caps-row">
                <span>Direct Audio Playback</span>
                <strong>{selectedProvider.capabilities.playback ? 'Permitted' : 'Disabled / Metadata Only'}</strong>
              </div>
              <div className="caps-row">
                <span>Audio Caching</span>
                <strong>{selectedProvider.capabilities.caching ? 'Permitted' : 'Prohibited by Terms'}</strong>
              </div>
              <div className="caps-row">
                <span>Lyrics Delivery</span>
                <strong>{selectedProvider.capabilities.lyrics ? 'Permitted' : 'Not Supported'}</strong>
              </div>
            </div>

            <div className="provider-notes-box">
              <Lock size={15} />
              <span>{selectedProvider.notes}</span>
            </div>
          </div>
        </Modal>
      )}
    </section>
  );
}
