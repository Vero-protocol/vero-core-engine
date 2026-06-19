import React from "react";

interface CardProps {
  title?: React.ReactNode;
  description?: React.ReactNode;
  actions?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}

/**
 * Card — shared surface for dashboard panels. The header keeps a sticky
 * slot for action controls (e.g. CSV export, Probe all) so the body can
 * scroll without losing access to the buttons.
 */
export const Card: React.FC<CardProps> = ({
  title,
  description,
  actions,
  children,
  className = "",
}) => {
  return (
    <section
      className={
        "bg-white dark:bg-gray-800 rounded-xl shadow-sm border border-gray-200 dark:border-gray-700 " +
        "transition-colors duration-200 " +
        className
      }
    >
      {(title || actions) && (
        <header className="flex flex-wrap items-start justify-between gap-4 p-5 border-b border-gray-100 dark:border-gray-700/60">
          <div>
            {title && (
              <h2 className="text-base font-semibold text-gray-900 dark:text-gray-50">
                {title}
              </h2>
            )}
            {description && (
              <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
                {description}
              </p>
            )}
          </div>
          {actions && <div className="flex items-center gap-2">{actions}</div>}
        </header>
      )}
      <div className="p-5">{children}</div>
    </section>
  );
};
