type AuditEntry = {
  sequence: number;
  ledger: number;
  timestamp: string;
  operation: string;
  author: string;
  payload: string;
  prevHash: string;
  stateHash: string;
  status: 'verified' | 'anchored' | 'pending';
};

const AUDIT_ENTRIES: AuditEntry[] = [
  {
    sequence: 401,
    ledger: 9441201,
    timestamp: '2026-06-19 08:14 UTC',
    operation: 'Treasury rebalance',
    author: 'GDN-4',
    payload: 'rebalance:hot-wallet',
    prevHash: '000000000000',
    stateHash: 'f3a91c72b4de',
    status: 'verified',
  },
  {
    sequence: 402,
    ledger: 9441209,
    timestamp: '2026-06-19 08:21 UTC',
    operation: 'Proposal approved',
    author: 'SIG-2',
    payload: 'gov:approve#118',
    prevHash: 'f3a91c72b4de',
    stateHash: '8b275de91a44',
    status: 'verified',
  },
  {
    sequence: 403,
    ledger: 9441216,
    timestamp: '2026-06-19 08:27 UTC',
    operation: 'Bridge settlement',
    author: 'REL-1',
    payload: 'bridge:settle#9241',
    prevHash: '8b275de91a44',
    stateHash: '1ed0c3a7fe28',
    status: 'anchored',
  },
  {
    sequence: 404,
    ledger: 9441220,
    timestamp: '2026-06-19 08:31 UTC',
    operation: 'Circuit reset',
    author: 'GDN-1',
    payload: 'breaker:reset',
    prevHash: '1ed0c3a7fe28',
    stateHash: 'af74d17ce905',
    status: 'pending',
  },
];

const STATUS_STYLES: Record<AuditEntry['status'], { label: string; className: string }> = {
  verified: {
    label: 'Verified',
    className:
      'bg-emerald-100 text-emerald-800 dark:bg-emerald-500/15 dark:text-emerald-200',
  },
  anchored: {
    label: 'Anchored',
    className: 'bg-sky-100 text-sky-800 dark:bg-sky-500/15 dark:text-sky-200',
  },
  pending: {
    label: 'Pending',
    className: 'bg-amber-100 text-amber-800 dark:bg-amber-500/15 dark:text-amber-200',
  },
};

const shortHash = (value: string) => `${value.slice(0, 6)}...${value.slice(-4)}`;

export function AuditTrailGraph() {
  const verifiedCount = AUDIT_ENTRIES.filter((entry) => entry.status === 'verified').length;

  return (
    <section
      className="bg-gray-50 dark:bg-gray-800 p-6 rounded-xl shadow-sm border dark:border-gray-700"
      aria-labelledby="audit-trail-heading"
    >
      <div className="flex flex-col gap-4 md:flex-row md:items-end md:justify-between">
        <div>
          <h2 id="audit-trail-heading" className="text-lg font-semibold mb-1">
            Audit trail
          </h2>
          <p className="text-sm opacity-80 max-w-2xl">
            Chained commitments are rendered in sequence so guardians can inspect who
            authored each transition, which payload moved state forward, and whether the
            link has already been verified on-chain.
          </p>
        </div>

        <div className="grid grid-cols-2 gap-3 text-sm md:min-w-[18rem]">
          <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white/80 dark:bg-gray-900/40 px-3 py-2">
            <div className="opacity-70">Chain depth</div>
            <div className="text-xl font-semibold">{AUDIT_ENTRIES.length} commits</div>
          </div>
          <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white/80 dark:bg-gray-900/40 px-3 py-2">
            <div className="opacity-70">Verified links</div>
            <div className="text-xl font-semibold">{verifiedCount} confirmed</div>
          </div>
        </div>
      </div>

      <div className="mt-6 rounded-2xl border border-gray-200 dark:border-gray-700 bg-white/80 dark:bg-gray-900/50 p-4">
        <div
          className="grid gap-4 md:grid-cols-2 xl:grid-cols-4"
          role="list"
          aria-label="Audit history graph"
        >
          {AUDIT_ENTRIES.map((entry, index) => {
            const status = STATUS_STYLES[entry.status];
            const hasNext = index < AUDIT_ENTRIES.length - 1;

            return (
              <article
                key={entry.sequence}
                role="listitem"
                className="relative rounded-xl border border-gray-200 dark:border-gray-700 bg-gray-50/80 dark:bg-gray-950/40 p-4"
              >
                {hasNext && (
                  <div
                    aria-hidden="true"
                    className="hidden xl:block absolute top-14 left-full h-0.5 w-4 bg-gradient-to-r from-sky-500 to-cyan-300"
                  />
                )}

                <div className="flex items-start justify-between gap-3">
                  <div>
                    <div className="text-xs uppercase tracking-[0.2em] opacity-60">
                      Seq {entry.sequence}
                    </div>
                    <h3 className="mt-1 text-base font-semibold">{entry.operation}</h3>
                  </div>
                  <span className={`rounded-full px-2.5 py-1 text-xs font-medium ${status.className}`}>
                    {status.label}
                  </span>
                </div>

                <dl className="mt-4 space-y-3 text-sm">
                  <div className="flex items-center justify-between gap-3">
                    <dt className="opacity-60">Ledger</dt>
                    <dd className="font-medium">{entry.ledger}</dd>
                  </div>
                  <div className="flex items-center justify-between gap-3">
                    <dt className="opacity-60">Author</dt>
                    <dd className="font-medium">{entry.author}</dd>
                  </div>
                  <div>
                    <dt className="opacity-60 mb-1">Payload</dt>
                    <dd className="font-mono text-xs rounded-md bg-gray-900 text-cyan-100 px-2 py-1.5 overflow-x-auto">
                      {entry.payload}
                    </dd>
                  </div>
                  <div>
                    <dt className="opacity-60 mb-1">Link</dt>
                    <dd className="text-xs font-mono leading-5">
                      <span className="opacity-70">prev</span> {shortHash(entry.prevHash)}
                      <br />
                      <span className="opacity-70">hash</span> {shortHash(entry.stateHash)}
                    </dd>
                  </div>
                </dl>

                <div className="mt-4 flex items-center justify-between gap-3 border-t border-gray-200 dark:border-gray-700 pt-3 text-xs opacity-70">
                  <span>{entry.timestamp}</span>
                  <span>{hasNext ? 'chains forward' : 'latest head'}</span>
                </div>
              </article>
            );
          })}
        </div>
      </div>
    </section>
  );
}

export default AuditTrailGraph;
