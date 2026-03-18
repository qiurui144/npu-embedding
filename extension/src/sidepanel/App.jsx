import { h, render } from 'preact';
import { useState } from 'preact/hooks';
import SearchPage from './pages/SearchPage.jsx';
import TimelinePage from './pages/TimelinePage.jsx';
import StatusPage from './pages/StatusPage.jsx';
import './sidepanel.css';

const TABS = [
  { id: 'search', label: '搜索' },
  { id: 'timeline', label: '时间线' },
  { id: 'status', label: '状态' },
];

const PAGE_MAP = { search: SearchPage, timeline: TimelinePage, status: StatusPage };

function App() {
  const [tab, setTab] = useState('search');
  const Page = PAGE_MAP[tab];

  return (
    <div class="sp-container">
      <div class="sp-tabs">
        {TABS.map((t) => (
          <button
            key={t.id}
            class={`sp-tab${tab === t.id ? ' sp-tab--active' : ''}`}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>
      <div class="sp-page">
        <Page />
      </div>
    </div>
  );
}

render(<App />, document.getElementById('app'));
