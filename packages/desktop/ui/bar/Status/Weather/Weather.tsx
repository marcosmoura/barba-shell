import { Button } from '@/components/Button';
import { Icon } from '@/components/Icon';
import { ScrollingLabel } from '@/components/ScrollingLabel';
import { Surface } from '@/components/Surface';

import { useWeather } from './Weather.state';
import * as styles from './Weather.styles';

export const Weather = () => {
  const { label, icon, onWeatherClick } = useWeather();

  return (
    <Surface as={Button} onClick={onWeatherClick}>
      <Icon icon={icon} />
      <ScrollingLabel className={styles.label}>{label}</ScrollingLabel>
    </Surface>
  );
};
