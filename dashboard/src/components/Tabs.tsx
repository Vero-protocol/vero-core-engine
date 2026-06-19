import React from "react";

export interface TabDescriptor {
  id: string;
  label: string;
}

interface TabsProps {
  tabs: readonly TabDescriptor[];
  activeId: string;
  onChange: (id: string) => void;
}

/**
 * Tabs — minimal, accessible tab nav with arrow-key support between tabs.
 */
export const Tabs: React.FC<TabsProps> = ({ tabs, activeId, onChange }) => {
  const onKeyDown = (e: React.KeyboardEvent<HTMLButtonElement>, idx: number) => {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    e.preventDefault();
    const dir = e.key === "ArrowRight" ? 1 : -1;
    const next = (idx + dir + tabs.length) % tabs.length;
    onChange(tabs[next].id);
  };

  return (
    <div role="tablist" aria-label="Dashboard sections" className="inline-flex p-1 bg-gray-100 dark:bg-gray-800/80 rounded-lg border border-gray-200 dark:border-gray-700">
      {tabs.map((tab, idx) => {
        const isActive = tab.id === activeId;
        return (
          <button
            key={tab.id}
            role="tab"
            aria-selected={isActive}
            aria-controls={`panel-${tab.id}`}
            id={`tab-${tab.id}`}
            tabIndex={isActive ? 0 : -1}
            onKeyDown={(e) => onKeyDown(e, idx)}
            onClick={() => onChange(tab.id)}
            className={
              "px-4 py-1.5 text-sm font-medium rounded-md transition-colors duration-150 focus:outline-none focus:ring-2 focus:ring-blue-500/60 " +
              (isActive
                ? "bg-white dark:bg-gray-900 text-gray-900 dark:text-gray-50 shadow-sm"
                : "text-gray-600 hover:text-gray-900 dark:text-gray-300 dark:hover:text-white")
            }
          >
            {tab.label}
          </button>
        );
      })}
    </div>
  );
};
