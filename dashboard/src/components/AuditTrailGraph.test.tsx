import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { AuditTrailGraph } from './AuditTrailGraph';

describe('AuditTrailGraph', () => {
  it('renders the audit history graph and visible chain entries', () => {
    render(<AuditTrailGraph />);

    expect(screen.getByRole('heading', { name: /audit trail/i })).toBeInTheDocument();
    expect(screen.getByLabelText(/audit history graph/i)).toBeInTheDocument();
    expect(screen.getByText(/4 commits/i)).toBeInTheDocument();
    expect(screen.getByText(/treasury rebalance/i)).toBeInTheDocument();
    expect(screen.getByText(/proposal approved/i)).toBeInTheDocument();
    expect(screen.getByText(/bridge:settle#9241/i)).toBeInTheDocument();
    expect(screen.getByText(/latest head/i)).toBeInTheDocument();
  });
});
