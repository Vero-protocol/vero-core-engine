import { ThemeToggle } from './components/ThemeToggle';
import { GuardianConfigForm } from './components/GuardianConfigForm';
import './index.css';

function App() {
  return (
    <div className="min-h-screen w-full bg-white dark:bg-gray-900 text-gray-900 dark:text-white transition-colors duration-200">
      <header className="p-4 flex justify-between items-center border-b dark:border-gray-800">
        <h1 className="text-xl font-bold">Guardian Dashboard</h1>
        <ThemeToggle />
      </header>
      <main className="p-8 max-w-4xl mx-auto space-y-6">
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

        <section
          className="bg-gray-50 dark:bg-gray-800 p-6 rounded-xl shadow-sm border dark:border-gray-700"
          aria-labelledby="theme-demo-heading"
        >
          <h2 id="theme-demo-heading" className="text-lg font-semibold mb-4">
            Theme persistence
          </h2>
          <p className="mb-4 opacity-80 text-sm">
            Click the toggle in the top-right to switch between Light and
            Dark modes. Your preference is saved in{' '}
            <strong>localStorage</strong> and persists across reloads.
          </p>
          <div className="flex gap-4" aria-hidden="true">
            <div className="h-20 w-20 rounded bg-blue-500 flex items-center justify-center text-white">
              Blue
            </div>
            <div className="h-20 w-20 rounded bg-green-500 flex items-center justify-center text-white">
              Green
            </div>
            <div className="h-20 w-20 rounded bg-red-500 flex items-center justify-center text-white">
              Red
            </div>
          </div>
        </section>
      </main>
    </div>
  );
}

export default App;
