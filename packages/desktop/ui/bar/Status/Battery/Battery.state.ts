import { useCallback, useMemo } from 'react';

import {
  BatteryEmptyIcon,
  BatteryChargingIcon,
  BatteryFullIcon,
  BatteryMedium02Icon,
  BatteryMediumIcon,
  BatteryLowIcon,
} from '@hugeicons/core-free-icons';
import { useQuery } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';

import { colors } from '@/design-system';

import type { BatteryState, BatteryData, BatteryInfo } from './Battery.types';

const CHARGING_POLLING_INTERVAL = 30 * 1000; // 30 seconds
const DISCHARGING_POLLING_INTERVAL = 2 * 60 * 1000; // 2 minutes

const getPollingInterval = (state?: BatteryState) => {
  return state === 'Charging' ? CHARGING_POLLING_INTERVAL : DISCHARGING_POLLING_INTERVAL;
};

const fetchBattery = async (): Promise<BatteryData | null> => {
  const battery = await invoke<BatteryInfo>('get_battery_info');

  if (!battery) {
    return null;
  }

  const { percentage, state } = battery;

  return {
    label: state === 'Full' ? '100%' : `${percentage}% (${state})`,
    percentage,
    state,
  };
};

export const useBattery = () => {
  const { data: battery } = useQuery({
    queryKey: ['battery'],
    queryFn: fetchBattery,
    refetchInterval: ({ state }) => getPollingInterval(state.data?.state),
    refetchOnMount: true,
  });

  const { state, percentage, label } = battery || {};

  const icon = useMemo(() => {
    if (typeof percentage !== 'number') {
      return BatteryEmptyIcon;
    }

    if (state === 'Charging') {
      return BatteryChargingIcon;
    }

    switch (true) {
      case percentage === 100:
        return BatteryFullIcon;
      case percentage >= 75:
        return BatteryMedium02Icon;
      case percentage >= 50:
        return BatteryMediumIcon;
      case percentage >= 25:
        return BatteryLowIcon;
      default:
        return BatteryEmptyIcon;
    }
  }, [state, percentage]);

  const color = useMemo(() => {
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
  }, [state]);

  const onBatteryClick = useCallback(() => invoke('open_app', { name: 'Battery' }), []);

  return { percentage, label, icon, color, onBatteryClick };
};
