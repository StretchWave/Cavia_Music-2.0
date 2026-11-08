import { Database, HardDrive, Music2, Radio, ShieldCheck } from 'lucide-react';
import type { ReactNode } from 'react';
import type { PlatformHealth } from '../../lib/platform';

export function SettingsView({ health }: { health: PlatformHealth | null }) {
  return <section className="subpage settings-page">
    <div className="eyebrow"><span className="eyebrow-line" />MAKE IT YOURS</div>
    <h1>Settings &<br /><em>system.</em></h1>
    <div className="settings-grid">
      <SettingCard icon={<Database size={18} />} title="Local database" value={health?.database_connected ? 'Connected' : 'Not verified'} detail="SQLite · Local-first storage" ok={!!health?.database_connected} />
      <SettingCard icon={<Music2 size={18} />} title="Playback engine" value="Not integrated" detail="Desktop and native mobile adapters are planned." ok={false} />
      <SettingCard icon={<Radio size={18} />} title="Music providers" value="Not configured" detail="No catalogue source is enabled in this foundation." ok={false} />
      <SettingCard icon={<HardDrive size={18} />} title="Offline cache" value="Not initialized" detail="Cache policy and storage limits are not active yet." ok={false} />
      <SettingCard icon={<ShieldCheck size={18} />} title="Privacy model" value="Local-first" detail="No account or cloud sync is required for the core library." ok />
    </div>
  </section>;
}

function SettingCard({ icon, title, value, detail, ok }: { icon: ReactNode; title: string; value: string; detail: string; ok: boolean }) {
  return <article className="setting-card"><div className="setting-icon">{icon}</div><div className="setting-copy"><strong>{title}</strong><span>{detail}</span></div><div className={`setting-value ${ok ? 'value-ok' : ''}`}><i />{value}</div></article>;
}
