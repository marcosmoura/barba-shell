import { useCallback } from 'react';

import { useQuery } from '@tanstack/react-query';

import { Button } from '@/components/Button';
import { Icon } from '@/components/Icon';
import { ScrollingLabel } from '@/components/ScrollingLabel';
import { Surface } from '@/components/Surface';
import { useMediaQuery } from '@/hooks';
import { LAPTOP_MEDIA_QUERY } from '@/utils/media-query';

import {
  fetchLocation,
  fetchWeather,
  getWeatherConfig,
  getWeatherIcon,
  getWeatherLabel,
  openWeatherApp,
} from './Weather.service';
import * as styles from './Weather.styles';

const queryOptions = {
  refetchInterval: 20 * 60 * 1000, // 20 minutes
};

export const Weather = () => {
  const isLaptopScreen = useMediaQuery(LAPTOP_MEDIA_QUERY);
  const { data: config } = useQuery({
    queryKey: ['weatherConfig'],
    queryFn: getWeatherConfig,
    staleTime: Infinity, // Config doesn't change during runtime
  });
  const { data: location } = useQuery({
    ...queryOptions,
    queryKey: ['location', config?.defaultLocation],
    queryFn: () => fetchLocation(config!.defaultLocation),
    enabled: !!config,
  });
  const { data: weather } = useQuery({
    ...queryOptions,
    queryKey: ['weather', location, config?.visualCrossingApiKey],
    queryFn: () => fetchWeather(config!.visualCrossingApiKey, location!, config!.defaultLocation),
    enabled: !!config?.visualCrossingApiKey && !!location,
  });

  const onWeatherClick = useCallback(() => openWeatherApp(), []);

  if (!weather) {
    return null;
  }

  const { currentConditions } = weather;

  return (
    <Surface as={Button} onClick={onWeatherClick}>
      <Icon icon={getWeatherIcon(currentConditions)} />
      <ScrollingLabel className={styles.label}>
        {!weather ? 'Loading weather...' : getWeatherLabel(currentConditions, isLaptopScreen)}
      </ScrollingLabel>
    </Surface>
  );
};
