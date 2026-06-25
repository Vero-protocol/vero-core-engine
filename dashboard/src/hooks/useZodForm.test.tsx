import { describe, it, expect, vi } from 'vitest';
import type React from 'react';
import { act, renderHook } from '@testing-library/react';
import { z } from 'zod';
import { useZodForm } from './useZodForm';

const schema = z.object({
  name: z.string().min(2, 'Name must be at least 2 characters'),
  age: z.coerce.number().int().min(0).max(150),
});

describe('useZodForm', () => {
  it('starts with initial values', () => {
    const { result } = renderHook(() =>
      useZodForm({ schema, initialValues: { name: '', age: '' } }),
    );
    expect(result.current.values).toEqual({ name: '', age: '' });
    expect(result.current.isValid).toBe(false);
    expect(result.current.hasAttemptedSubmit).toBe(false);
  });

  it('updates values via setValue', () => {
    const { result } = renderHook(() =>
      useZodForm({ schema, initialValues: { name: '', age: '' } }),
    );
    act(() => {
      result.current.setValue('name', 'Gbenga');
    });
    expect(result.current.values.name).toBe('Gbenga');
  });

  it('keeps errors hidden until a field is blurred or submit is attempted', () => {
    const { result } = renderHook(() =>
      useZodForm({ schema, initialValues: { name: '', age: '' } }),
    );
    expect(result.current.visibleErrors.name).toBeUndefined();

    act(() => {
      result.current.handleBlur('name');
    });
    expect(result.current.visibleErrors.name).toBeDefined();
  });

  it('blocks onValid when validation fails and marks submit-attempted', async () => {
    const onValid = vi.fn();
    const { result } = renderHook(() =>
      useZodForm({ schema, initialValues: { name: '', age: '' } }),
    );

    await act(async () => {
      await result.current.handleSubmit(onValid)();
    });

    expect(onValid).not.toHaveBeenCalled();
    expect(result.current.hasAttemptedSubmit).toBe(true);
    expect(result.current.visibleErrors.name).toBeDefined();
  });

  it('calls onValid with parsed values when valid', async () => {
    const onValid = vi.fn();
    const { result } = renderHook(() =>
      useZodForm({
        schema,
        initialValues: { name: 'Gbenga', age: '30' },
      }),
    );

    await act(async () => {
      await result.current.handleSubmit(onValid)();
    });

    expect(onValid).toHaveBeenCalledTimes(1);
    expect(onValid).toHaveBeenCalledWith({ name: 'Gbenga', age: 30 });
  });

  it('preventDefault is called when a FormEvent is passed', async () => {
    const onValid = vi.fn();
    const { result } = renderHook(() =>
      useZodForm({
        schema,
        initialValues: { name: 'Gbenga', age: '30' },
      }),
    );

    const preventDefault = vi.fn();
    const fakeEvent = { preventDefault } as unknown as React.FormEvent;

    await act(async () => {
      await result.current.handleSubmit(onValid)(fakeEvent);
    });

    expect(preventDefault).toHaveBeenCalled();
  });

  it('resets to initial values and clears touched/submitAttempted', async () => {
    const { result } = renderHook(() =>
      useZodForm({ schema, initialValues: { name: 'Gbenga', age: '30' } }),
    );

    act(() => {
      result.current.setValue('name', 'Something Else');
      result.current.handleBlur('name');
    });
    expect(result.current.values.name).toBe('Something Else');

    act(() => {
      result.current.reset();
    });

    expect(result.current.values).toEqual({ name: 'Gbenga', age: '30' });
    expect(result.current.hasAttemptedSubmit).toBe(false);
    expect(result.current.visibleErrors.name).toBeUndefined();
  });

  it('reset accepts a new value set', () => {
    const { result } = renderHook(() =>
      useZodForm({ schema, initialValues: { name: '', age: '' } }),
    );
    act(() => {
      result.current.reset({ name: 'New', age: '10' });
    });
    expect(result.current.values).toEqual({ name: 'New', age: '10' });
    expect(result.current.isValid).toBe(true);
  });
});
