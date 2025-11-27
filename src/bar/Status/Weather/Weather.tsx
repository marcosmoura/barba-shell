import { useCallback } from 'react';

import { useQuery } from '@tanstack/react-query';

import { Button } from '@/components/Button';
import { Icon } from '@/components/Icon';
import { Surface } from '@/components/Surface';
import { useMediaQuery } from '@/hooks';
import { LAPTOP_MEDIA_QUERY } from '@/utils';

import {
  fetchLocation,
  fetchWeather,
  getWeatherIcon,
  getWeatherLabel,
  openWeatherApp,
} from './Weather.service';

const queryOptions = {
  refetchInterval: 20 * 60 * 1000, // 20 minutes
};

export const Weather = () => {
  const isLaptopScreen = useMediaQuery(LAPTOP_MEDIA_QUERY);
  const { data: location } = useQuery({
    ...queryOptions,
    queryKey: ['location'],
    queryFn: fetchLocation,
  });
  const { data: weather } = useQuery({
    ...queryOptions,
    queryKey: ['weather', location],
    queryFn: () => fetchWeather(location),
    enabled: !!location,
  });

  const onWeatherClick = useCallback(() => openWeatherApp(), []);

  if (!weather) {
    return null;
  }

  const { currentConditions } = weather;

  return (
    <Surface as={Button} onClick={onWeatherClick}>
      <Icon icon={getWeatherIcon(currentConditions)} />
      <span>
        {!weather ? 'Loading weather...' : getWeatherLabel(currentConditions, isLaptopScreen)}
      </span>
    </Surface>
  );
};
