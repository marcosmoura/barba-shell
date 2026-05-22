import { colors } from '@/design-system';
import { useWidgetToggle } from '@/hooks';
import { getBatteryIcon, useBatteryStore } from '@/stores/BatteryStore';

import type { BatteryState } from './Battery.types';

function getColor(state?: string): string {
  switch (state) {
    case 'Charging':
      return colors.green;
    case 'Discharging':
      return colors.yellow;
    case 'Empty':
      return colors.red;
    default:
      return colors.text;
  }
}

function getLabel(percentage?: number, state?: string): string {
  if (typeof percentage !== 'number' || !state) {
    return 'Loading...';
  }

  if (state === 'Full') {
    return '100%';
  }

  if (state === 'Unknown') {
    return `${percentage}%`;
  }

  return `${percentage}% (${state})`;
}

export function useBattery(): BatteryState {
  const { ref, onClick } = useWidgetToggle('battery');

  const { battery } = useBatteryStore();
  const { state, percentage } = battery || {};

  return {
    icon: getBatteryIcon(percentage, state),
    label: getLabel(percentage, state),
    color: getColor(state),
    percentage,
    ref,
    onClick,
  };
}
