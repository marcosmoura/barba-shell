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
import { invoke } from '@tauri-apps/api/core';

import type {
  CurrentConditions,
  IpApiResponse,
  IpInfoResponse,
  WeatherConfig,
  WeatherData,
} from './Weather.types';

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

export const getWeatherConfig = (): Promise<WeatherConfig> => {
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

export const fetchLocation = async (defaultLocation: string): Promise<string> => {
  const location = (await fetchIpApiLocation()) ?? (await fetchIpInfoLocation());

  return location || defaultLocation;
};

export const fetchWeather = async (
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

export const openWeatherApp = () => invoke('open_app', { name: 'Weather' });

export const getWeatherIcon = (conditions?: CurrentConditions) => {
  const defaultIcon = iconMap['clear-day'];

  if (!conditions) {
    return defaultIcon;
  }

  return iconMap[conditions.icon] ?? defaultIcon;
};

export const getWeatherLabel = (
  currentConditions?: CurrentConditions,
  isLaptopScreen?: boolean,
): string => {
  if (!currentConditions) {
    return '';
  }

  const feelsLike = Math.ceil(currentConditions.feelslike || 0);
  const condition = currentConditions.conditions || '';

  if (isLaptopScreen) {
    return `${feelsLike}°C`;
  }

  return `${feelsLike}°C (${condition})`;
};
