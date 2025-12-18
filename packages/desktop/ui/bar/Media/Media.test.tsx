import { describe, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';

import { createQueryClientWrapper, createTestQueryClient } from '@/tests/utils';

import { Media } from './Media';

describe('Media Component', () => {
  test('renders nothing when no media is playing', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['media'], null);

    const { container } = await render(<Media />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(container.querySelector('[data-test-id="media-container"]')).toBeNull();
    });

    queryClient.clear();
  });

  test('renders media container when media is playing', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['media'], {
      label: 'Test Song - Test Artist',
      prefix: '',
      bundleIdentifier: 'com.spotify.client',
      artwork: null,
    });

    const { getByTestId } = await render(<Media />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(getByTestId('media-container')).toBeDefined();
    });

    queryClient.clear();
  });

  test('renders media label', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['media'], {
      label: 'Bohemian Rhapsody - Queen',
      prefix: '',
      bundleIdentifier: 'com.spotify.client',
      artwork: null,
    });

    const { getByText } = await render(<Media />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(getByText('Bohemian Rhapsody - Queen')).toBeDefined();
    });

    queryClient.clear();
  });

  test('renders paused prefix when media is paused', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['media'], {
      label: 'Test Song - Test Artist',
      prefix: 'Paused: ',
      bundleIdentifier: 'com.spotify.client',
      artwork: null,
    });

    const { getByText } = await render(<Media />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(getByText('Paused:')).toBeDefined();
    });

    queryClient.clear();
  });
});
