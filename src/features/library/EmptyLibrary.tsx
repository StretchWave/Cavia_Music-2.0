import { ArrowUpRight, FolderPlus, Sparkles } from 'lucide-react';

export function EmptyLibrary() {
  return <section className="empty-library">
    <div className="empty-art" aria-hidden="true">
      <div className="art-orbit orbit-one" /><div className="art-orbit orbit-two" /><div className="art-orbit orbit-three" />
      <div className="art-disc"><div className="disc-groove groove-one" /><div className="disc-groove groove-two" /><div className="disc-groove groove-three" /><div className="disc-label"><span>藏</span><i /></div></div>
      <div className="art-glint glint-one" /><div className="art-glint glint-two" />
    </div>
    <div className="empty-copy">
      <div className="eyebrow"><Sparkles size={13} /> A BLANK CANVAS, NOT A BLANK EXPERIENCE</div>
      <h2>Your library is<br /><em>yours to shape.</em></h2>
      <p>Cavia doesn't invent a catalogue for you. Bring in your own music and build a collection that feels like yours.</p>
      <div className="empty-actions"><button className="primary-action" disabled title="Local audio import is the next implementation milestone"><FolderPlus size={16} /> Add music <span className="coming-tag">NEXT MILESTONE</span></button><button className="secondary-action" disabled title="Provider integrations are not connected yet">Explore sources <ArrowUpRight size={15} /></button></div>
      <div className="empty-footnote"><span className="footnote-mark" /> No demo tracks. No fabricated playlists. Just your music.</div>
    </div>
  </section>;
}
