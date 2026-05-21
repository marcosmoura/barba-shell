import { useWidgetToggle } from '@/hooks';
import { getWeatherIcon, useWeatherStore } from '@/stores/WeatherStore';

export const useWeather = () => {
  const { ref, onClick } = useWidgetToggle('weather');
  const { weather, isLoading, isConfigured } = useWeatherStore();

  const currentConditions = weather?.currentConditions;

  return {
    label:
      isLoading || !currentConditions
        ? 'Loading weather...'
        : `${Math.ceil(currentConditions.feelslike || 0)}°C`,
    icon: getWeatherIcon(currentConditions?.icon ?? 'clearDay'),
    ref,
    onClick,
    isConfigured,
  };
};
