import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { GuardianConfigForm } from './GuardianConfigForm';

const renderForm = (props: React.ComponentProps<typeof GuardianConfigForm> = {}) =>
  render(<GuardianConfigForm {...props} />);

describe('GuardianConfigForm', () => {
  it('renders all four labelled fields', () => {
    renderForm();
    expect(screen.getByLabelText(/RPC endpoint/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/Signer threshold/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/Time-lock/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/Notification webhook URL/i)).toBeInTheDocument();
  });

  it('blocks submission when the form is empty', async () => {
    const onSave = vi.fn();
    renderForm({ onSave });

    fireEvent.click(screen.getByTestId('guardian-config-submit'));

    await waitFor(() => {
      expect(onSave).not.toHaveBeenCalled();
    });
    // Surfaced inline errors after submit attempt
    expect(screen.getAllByRole('alert').length).toBeGreaterThan(0);
  });

  it('blocks http RPC endpoint with an inline error', async () => {
    const onSave = vi.fn();
    renderForm({ onSave });

    const rpc = screen.getByLabelText(/RPC endpoint/i);
    fireEvent.change(rpc, { target: { value: 'http://example.com' } });
    fireEvent.blur(rpc);
    expect(screen.getByRole('alert')).toHaveTextContent(/HTTPS/i);

    fireEvent.click(screen.getByTestId('guardian-config-submit'));
    await waitFor(() => {
      expect(onSave).not.toHaveBeenCalled();
    });
  });

  it('submits valid values to onSave and shows the success banner', async () => {
    const onSave = vi.fn();
    renderForm({ onSave });

    fireEvent.change(screen.getByLabelText(/RPC endpoint/i), {
      target: { value: 'https://soroban-testnet.stellar.org' },
    });
    fireEvent.change(screen.getByLabelText(/Signer threshold/i), {
      target: { value: '3' },
    });
    fireEvent.change(screen.getByLabelText(/Time-lock/i), {
      target: { value: '720' },
    });
    fireEvent.change(screen.getByLabelText(/Notification webhook URL/i), {
      target: { value: 'https://hooks.example.com/guardian' },
    });

    fireEvent.click(screen.getByTestId('guardian-config-submit'));

    await waitFor(() => {
      expect(onSave).toHaveBeenCalledTimes(1);
    });

    const savedArg = onSave.mock.calls[0][0];
    expect(savedArg).toEqual({
      rpcEndpoint: 'https://soroban-testnet.stellar.org',
      signerThreshold: 3,
      timelockSeconds: 720,
      webhookUrl: 'https://hooks.example.com/guardian',
    });

    expect(screen.getByTestId('guardian-config-success')).toBeInTheDocument();
  });

  it('resets form state when Reset is clicked', () => {
    renderForm();

    const rpc = screen.getByLabelText(/RPC endpoint/i);
    fireEvent.change(rpc, { target: { value: 'https://example.com' } });
    expect(rpc).toHaveValue('https://example.com');

    fireEvent.click(screen.getByRole('button', { name: /reset/i }));
    expect(rpc).toHaveValue('');
  });
});
