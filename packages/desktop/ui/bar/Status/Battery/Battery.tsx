import { Button } from '@/components/Button';
import { Icon } from '@/components/Icon';
import { Surface } from '@/components/Surface';

import { useBattery } from './Battery.state';

export const Battery = () => {
  const { onBatteryClick, percentage, label, icon, color } = useBattery();

  if (!percentage) {
    return null;
  }

  return (
    <Surface as={Button} onClick={onBatteryClick}>
      <Icon icon={icon} color={color} />
      <span>{label}</span>
    </Surface>
  );
};
