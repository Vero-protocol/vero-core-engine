import { ThemeToggle } from './components/ThemeToggle';
import './index.css';

function App() {
  return (
    <div className="min-h-screen w-full bg-white dark:bg-gray-900 text-gray-900 dark:text-white transition-colors duration-200">
      <header className="p-4 flex justify-between items-center border-b dark:border-gray-800">
        <h1 className="text-xl font-bold">Guardian Dashboard</h1>
        <ThemeToggle />
      </header>
      <main className="p-8 max-w-4xl mx-auto">
        <div className="bg-gray-50 dark:bg-gray-800 p-6 rounded-xl shadow-sm border dark:border-gray-700">
          <h2 className="text-lg font-semibold mb-4">Theme Persistence Demo</h2>
          <p className="mb-4 opacity-80">
            Click the toggle in the top-right to switch between Light and Dark modes.
            Your preference will be saved in <strong>LocalStorage</strong> and persist across reloads.
          </p>
          <div className="flex gap-4">
            <div className="h-20 w-20 rounded bg-blue-500 flex items-center justify-center text-white">Blue</div>
            <div className="h-20 w-20 rounded bg-green-500 flex items-center justify-center text-white">Green</div>
            <div className="h-20 w-20 rounded bg-red-500 flex items-center justify-center text-white">Red</div>
          </div>
        </div>
      </main>
    </div>
  );
}

export default App;
