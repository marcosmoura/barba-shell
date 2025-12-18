import { describe, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';

import { createQueryClientWrapper, createTestQueryClient } from '@/tests/utils';

import { Spaces } from './Spaces';

describe('Spaces Component', () => {
  test('renders spaces container', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['hyprspace_workspaces'], ['terminal', 'coding', 'browser']);
    queryClient.setQueryData(['hyprspace_current_workspace'], 'terminal');
    queryClient.setQueryData(['focused_app'], 'Ghostty');

    const { getByTestId } = await render(<Spaces />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(getByTestId('spaces-container')).toBeDefined();
    });

    queryClient.clear();
  });

  test('renders with empty workspace list', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['hyprspace_workspaces'], []);
    queryClient.setQueryData(['hyprspace_current_workspace'], undefined);
    queryClient.setQueryData(['focused_app'], undefined);

    const { getByTestId } = await render(<Spaces />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(getByTestId('spaces-container')).toBeDefined();
    });

    queryClient.clear();
  });

  test('renders workspace buttons', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['hyprspace_workspaces'], ['terminal', 'coding']);
    queryClient.setQueryData(['hyprspace_current_workspace'], 'terminal');
    queryClient.setQueryData(['focused_app'], 'Ghostty');

    const { getByText } = await render(<Spaces />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      // Focused workspace shows capitalized name
      expect(getByText('Terminal')).toBeDefined();
    });

    queryClient.clear();
  });

  test('renders focused app name', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['hyprspace_workspaces'], ['terminal']);
    queryClient.setQueryData(['hyprspace_current_workspace'], 'terminal');
    queryClient.setQueryData(['focused_app'], 'Visual Studio Code');

    const { getByText } = await render(<Spaces />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(getByText('Visual Studio Code')).toBeDefined();
    });

    queryClient.clear();
  });
});
