import { useMemo } from 'react';

import { colors } from '@/design-system';
import { useWidgetToggle } from '@/hooks';
import { getBatteryIcon, useBatteryStore } from '@/stores/BatteryStore';

const getColor = (state?: string) => {
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
};

const getLabel = (percentage?: number, state?: string) => {
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
};

export const useBattery = () => {
  const { ref, onClick } = useWidgetToggle('battery');

  const { battery } = useBatteryStore();
  const { state, percentage } = battery || {};

  const batteryData = useMemo(
    () => ({
      icon: getBatteryIcon(percentage, state),
      label: getLabel(percentage, state),
      color: getColor(state),
      percentage,
    }),
    [percentage, state],
  );

  return { ...batteryData, ref, onClick };
};
