import { describe, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';

import { createQueryClientWrapper, createTestQueryClient } from '@/tests/utils';

import { Battery } from './Battery';

describe('Battery Component', () => {
  test('renders battery info when available', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['battery'], {
      percentage: 75,
      state: 'Discharging',
      label: '75% (Discharging)',
    });

    const { getByText } = await render(<Battery />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(getByText('75% (Discharging)')).toBeDefined();
    });

    queryClient.clear();
  });

  test('renders nothing when battery percentage is not available', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['battery'], null);

    const { container } = await render(<Battery />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      // Should not render anything
      expect(container.querySelector('button')).toBeNull();
    });

    queryClient.clear();
  });

  test('renders full battery label', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['battery'], {
      percentage: 100,
      state: 'Full',
      label: '100%',
    });

    const { getByText } = await render(<Battery />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(getByText('100%')).toBeDefined();
    });

    queryClient.clear();
  });

  test('renders charging battery label', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['battery'], {
      percentage: 50,
      state: 'Charging',
      label: '50% (Charging)',
    });

    const { getByText } = await render(<Battery />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(getByText('50% (Charging)')).toBeDefined();
    });

    queryClient.clear();
  });
});
