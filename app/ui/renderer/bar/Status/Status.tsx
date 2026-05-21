import { Stack } from '@/components/Stack';

import { Battery } from './Battery';
import { Clock } from './Clock';
import { Cpu } from './Cpu';
import { KeepAwake } from './KeepAwake';
import { Weather } from './Weather';
import { Wifi } from './Wifi';

export const Status = () => {
  return (
    <Stack data-testid="status-container">
      <Weather />
      <Cpu />
      <Battery />
      <KeepAwake />
      <Wifi />
      <Clock />
    </Stack>
  );
};
