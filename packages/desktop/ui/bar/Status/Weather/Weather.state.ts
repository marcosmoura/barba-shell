import { useCallback, useMemo } from 'react';

import {
  SnowIcon,
  SunCloudSnowIcon,
  MoonCloudSnowIcon,
  CloudAngledRainZapIcon,
  MoonAngledRainZapIcon,
  CloudAngledRainIcon,
  SunCloudAngledRainIcon,
  MoonCloudAngledRainIcon,
  CloudSlowWindIcon,
  FastWindIcon,
  CloudIcon,
  SunCloud02Icon,
  MoonCloudIcon,
  SunIcon,
  MoonIcon,
} from '@hugeicons/core-free-icons';
import type { IconSvgElement } from '@hugeicons/react';
import { useQuery } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';

import { useMediaQuery } from '@/hooks';
import { LAPTOP_MEDIA_QUERY } from '@/utils/media-query';

import type { WeatherConfig, IpApiResponse, IpInfoResponse, WeatherData } from './Weather.types';

const queryOptions = {
  refetchInterval: 20 * 60 * 1000, // 20 minutes
};

const API_URL = 'https://weather.visualcrossing.com/VisualCrossingWebServices/rest/services';
const API_ELEMENTS = 'name,address,resolvedAddress,feelslike,moonphase,conditions,description,icon';
const API_INCLUDE = 'alerts,current,fcst,days';

const iconMap: Record<string, IconSvgElement> = {
  snow: SnowIcon,
  'snow-showers-day': SunCloudSnowIcon,
  'snow-showers-night': MoonCloudSnowIcon,
  'thunder-rain': CloudAngledRainZapIcon,
  'thunder-showers-day': CloudAngledRainZapIcon,
  'thunder-showers-night': MoonAngledRainZapIcon,
  rain: CloudAngledRainIcon,
  'showers-day': SunCloudAngledRainIcon,
  'showers-night': MoonCloudAngledRainIcon,
  fog: CloudSlowWindIcon,
  wind: FastWindIcon,
  cloudy: CloudIcon,
  'partly-cloudy-day': SunCloud02Icon,
  'partly-cloudy-night': MoonCloudIcon,
  'clear-day': SunIcon,
  'clear-night': MoonIcon,
};

const getWeatherConfig = (): Promise<WeatherConfig> => {
  return invoke<WeatherConfig>('get_weather_config');
};

const buildLocationString = (parts: Array<string | undefined>): string =>
  parts.filter(Boolean).join(', ');

const fetchIpApiLocation = async (): Promise<string | undefined> => {
  try {
    const response = await fetch('https://ipapi.co/json/');

    if (!response.ok) {
      throw new Error('Failed to fetch from ipapi.co');
    }

    const data = (await response.json()) as IpApiResponse;
    const location = buildLocationString([data.city, data.country_name]);

    return location || undefined;
  } catch (error) {
    console.error(error);
    return undefined;
  }
};

const fetchIpInfoLocation = async (): Promise<string | undefined> => {
  try {
    const response = await fetch('https://ipinfo.io/json');

    if (!response.ok) {
      throw new Error('Failed to fetch from ipinfo.io');
    }

    const data = (await response.json()) as IpInfoResponse;
    const location = buildLocationString([data.city, data.country]);

    return location || undefined;
  } catch (error) {
    console.error(error);
    return undefined;
  }
};

const fetchLocation = async (defaultLocation: string): Promise<string> => {
  const location = (await fetchIpApiLocation()) ?? (await fetchIpInfoLocation());

  return location || defaultLocation;
};

const fetchWeather = async (
  apiKey: string,
  location: string,
  defaultLocation: string,
): Promise<WeatherData> => {
  const encodedLoc = encodeURIComponent(location || defaultLocation);
  const params = new URLSearchParams({
    key: apiKey,
    unitGroup: 'metric',
    elements: API_ELEMENTS,
    include: API_INCLUDE,
    iconSet: 'icons2',
    contentType: 'json',
  });

  const url = `${API_URL}/timeline/${encodedLoc}/today?${params.toString()}`;

  const response = await fetch(url);

  if (!response.ok) {
    throw new Error('Network response was not ok');
  }

  return await response.json();
};

const openWeatherApp = () => invoke('open_app', { name: 'Weather' });

export const useWeather = () => {
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

  const { currentConditions } = weather || {};

  const icon = useMemo(() => {
    const defaultIcon = iconMap['clear-day'];

    if (!currentConditions) {
      return defaultIcon;
    }

    return iconMap[currentConditions.icon] ?? defaultIcon;
  }, [currentConditions]);

  const label = useMemo((): string => {
    if (!currentConditions) {
      return 'Loading weather...';
    }

    const feelsLike = Math.ceil(currentConditions.feelslike || 0);
    const condition = currentConditions.conditions || '';

    if (isLaptopScreen) {
      return `${feelsLike}°C`;
    }

    return `${feelsLike}°C (${condition})`;
  }, [currentConditions, isLaptopScreen]);

  const onWeatherClick = useCallback(() => openWeatherApp(), []);

  return { label, icon, onWeatherClick };
};
