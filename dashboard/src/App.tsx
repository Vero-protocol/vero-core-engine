import { useState, type ReactNode } from 'react';
import { Shield } from 'lucide-react';
import { ThemeToggle } from './components/ThemeToggle';
import { GuardianConfigForm } from './components/GuardianConfigForm';
import { Tabs, type TabDescriptor } from './components/Tabs';
import { RpcSettings } from './components/RpcSettings';
import { LogExport } from './components/LogExport';
import { PerformanceStats } from './components/PerformanceStats';
import './index.css';

const TABS: readonly TabDescriptor[] = [
  { id: 'rpc', label: 'Custom RPC' },
  { id: 'logs', label: 'Log Export' },
  { id: 'stats', label: 'Performance' },
  { id: 'config', label: 'Guardian Config' },
];

/** Tab id → panel component lookup. Order/size independent. */
const PANELS: Record<string, () => ReactNode> = {
  rpc: () => <RpcSettings />,
  logs: () => <LogExport />,
  stats: () => <PerformanceStats />,
  config: () => (
    <section
      className="bg-gray-50 dark:bg-gray-800 p-6 rounded-xl shadow-sm border dark:border-gray-700"
      aria-labelledby="guardian-config-heading"
    >
      <h2 id="guardian-config-heading" className="text-lg font-semibold mb-1">
        Guardian configuration
      </h2>
      <p className="mb-4 opacity-80 text-sm">
        All inputs are validated client-side before they reach the
        relayer. Bad input is blocked and surfaced inline.
      </p>
      <GuardianConfigForm />
    </section>
  ),
};

function App() {
  const [activeTab, setActiveTab] = useState<string>(TABS[0].id);
  const RenderPanel = PANELS[activeTab] ?? PANELS[TABS[0].id];

  return (
    <div className="min-h-screen w-full bg-white dark:bg-gray-900 text-gray-900 dark:text-white transition-colors duration-200">
      <header className="p-4 flex flex-wrap items-center justify-between gap-3 border-b dark:border-gray-800">
        <div className="flex items-center gap-2">
          <div className="p-2 rounded-lg bg-blue-100 dark:bg-blue-900/40 text-blue-600 dark:text-blue-300">
            <Shield size={20} />
          </div>
          <div>
            <h1 className="text-lg font-bold">Guardian Dashboard</h1>
            <p className="text-[11px] text-gray-500 dark:text-gray-400 -mt-0.5">V-Zero Protocol · v0.1.0</p>
          </div>
        </div>
        <ThemeToggle />
      </header>

      <div className="px-6 pt-6">
        <Tabs tabs={TABS} activeId={activeTab} onChange={setActiveTab} />
      </div>

      <main className="p-6 max-w-5xl mx-auto space-y-6">
        <div
          role="tabpanel"
          id={`panel-${activeTab}`}
          aria-labelledby={`tab-${activeTab}`}
        >
          <RenderPanel />
        </div>
      </main>

      <footer className="px-6 pb-6 max-w-5xl mx-auto text-[11px] text-gray-500 dark:text-gray-400">
        Configuration is persisted locally. Restart the sampler or re-probe the network for fresh values.
      </footer>
    </div>
  );
}

export default App;
