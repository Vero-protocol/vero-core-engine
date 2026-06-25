import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Field } from './Field';

describe('Field', () => {
  it('renders the label and input by id linkage', () => {
    render(
      <Field
        label="RPC endpoint"
        value=""
        onChange={() => undefined}
      />,
    );
    const input = screen.getByLabelText(/RPC endpoint/i);
    expect(input).toBeInTheDocument();
    expect(input.tagName).toBe('INPUT');
  });

  it('invokes onChange with the new value', () => {
    const onChange = vi.fn();
    render(
      <Field
        label="RPC endpoint"
        value=""
        onChange={onChange}
      />,
    );
    fireEvent.change(screen.getByLabelText(/RPC endpoint/i), {
      target: { value: 'https://example.com' },
    });
    expect(onChange).toHaveBeenCalledWith('https://example.com');
  });

  it('renders hint text when no error is set', () => {
    render(
      <Field
        label="RPC endpoint"
        value=""
        onChange={() => undefined}
        hint="Must use HTTPS"
      />,
    );
    expect(screen.getByText(/Must use HTTPS/i)).toBeInTheDocument();
  });

  it('hides hint and shows error with role=alert when error is set', () => {
    render(
      <Field
        label="RPC endpoint"
        value="bad"
        onChange={() => undefined}
        error="Invalid URL"
        hint="Must use HTTPS"
      />,
    );
    const error = screen.getByRole('alert');
    expect(error).toHaveTextContent('Invalid URL');
    expect(screen.queryByText(/Must use HTTPS/i)).not.toBeInTheDocument();
  });

  it('sets aria-invalid only when an error is present', () => {
    const { rerender } = render(
      <Field
        label="RPC"
        value=""
        onChange={() => undefined}
      />,
    );
    expect(screen.getByLabelText(/RPC/i)).not.toHaveAttribute('aria-invalid');

    rerender(
      <Field
        label="RPC"
        value="bad"
        onChange={() => undefined}
        error="bad"
      />,
    );
    expect(screen.getByLabelText(/RPC/i)).toHaveAttribute('aria-invalid', 'true');
  });

  it('renders optional marker when optional=true', () => {
    render(
      <Field
        label="Webhook URL"
        value=""
        onChange={() => undefined}
        optional
      />,
    );
    expect(screen.getByText(/optional/i)).toBeInTheDocument();
  });
});
