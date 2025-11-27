import { describe, expect, test } from 'vitest';

import { truncate } from './truncate';

describe('truncate', () => {
  test('returns original text when length is within limit', () => {
    expect(truncate('desktop', 10)).toBe('desktop');
    expect(truncate('bar', 3)).toBe('bar');
  });

  test('appends ellipsis when text exceeds max length', () => {
    expect(truncate('barba app', 8)).toBe('barba...');
  });

  test('handles very small max lengths by returning only ellipsis', () => {
    expect(truncate('desktop', 2)).toBe('...');
    expect(truncate('desktop', 0)).toBe('...');
  });

  test('returns empty string when empty input is provided', () => {
    expect(truncate('', 5)).toBe('');
  });
});
