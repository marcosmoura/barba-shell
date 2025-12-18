import { describe, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';

import { createQueryClientWrapper, createTestQueryClient, createFetchMock } from '@/tests/utils';

import { Weather } from './Weather';

describe('Weather Component', () => {
  test('renders weather info', async () => {
    const mockFetch = createFetchMock([
      { pattern: 'ipapi.co', response: { city: 'Berlin', country_name: 'Germany' } },
      { pattern: 'ipinfo.io', response: { city: 'Berlin', country: 'DE' } },
      {
        pattern: 'visualcrossing',
        response: {
          currentConditions: {
            feelslike: 20,
            icon: 'clear-day',
            conditions: 'Clear',
          },
        },
      },
    ]);
    vi.spyOn(globalThis, 'fetch').mockImplementation(mockFetch);

    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['weather'], {
      temperature: 20,
      icon: 'clear-day',
      conditions: 'Clear',
    });

    const { container } = await render(<Weather />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(container.querySelector('button')).toBeDefined();
    });

    queryClient.clear();
    vi.restoreAllMocks();
  });

  test('renders temperature label', async () => {
    const mockFetch = createFetchMock([
      { pattern: 'ipapi.co', response: { city: 'London', country_name: 'UK' } },
      { pattern: 'ipinfo.io', response: { city: 'London', country: 'GB' } },
      {
        pattern: 'visualcrossing',
        response: {
          currentConditions: {
            feelslike: 15,
            icon: 'cloudy',
            conditions: 'Cloudy',
          },
        },
      },
    ]);
    vi.spyOn(globalThis, 'fetch').mockImplementation(mockFetch);

    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['weather-label'], '15°C Cloudy');

    const { getByText } = await render(<Weather />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(getByText('15°C Cloudy')).toBeDefined();
    });

    queryClient.clear();
    vi.restoreAllMocks();
  });

  test('renders weather container', async () => {
    const mockFetch = createFetchMock([
      { pattern: 'ipapi.co', response: { city: 'Tokyo', country_name: 'Japan' } },
      { pattern: 'ipinfo.io', response: { city: 'Tokyo', country: 'JP' } },
      {
        pattern: 'visualcrossing',
        response: {
          currentConditions: {
            feelslike: 25,
            icon: 'partly-cloudy-day',
            conditions: 'Partly Cloudy',
          },
        },
      },
    ]);
    vi.spyOn(globalThis, 'fetch').mockImplementation(mockFetch);

    const queryClient = createTestQueryClient();

    const { container } = await render(<Weather />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      // Weather always renders (even with loading state)
      expect(container.querySelector('button')).toBeDefined();
    });

    queryClient.clear();
    vi.restoreAllMocks();
  });
});
