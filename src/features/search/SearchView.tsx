import { Search, RadioTower } from 'lucide-react';

export function SearchView() {
  return <section className="subpage">
    <div className="eyebrow"><span className="eyebrow-line" />DISCOVER YOUR NEXT REPEAT</div>
    <h1>Find your<br /><em>frequency.</em></h1>
    <div className="search-box"><Search size={19} /><input aria-label="Search music" placeholder="Search tracks, artists, albums..." disabled /><kbd>⌘ K</kbd></div>
    <div className="search-state"><div className="search-state-icon"><RadioTower size={21} /></div><div><strong>Search providers aren't connected yet</strong><p>The search interface is ready for real provider adapters. Results will only appear when a source returns actual catalogue data.</p></div></div>
  </section>;
}
