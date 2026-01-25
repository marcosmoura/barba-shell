import { useDisableRightClick } from '@/hooks';
import { useTauri } from '@/hooks/useTauri';
import { MenubarEvents } from '@/types';

export const useBar = () => {
  const { data: menuHidden } = useTauri<boolean>({
    queryKey: ['menubar-visibility'],
    queryFn: async () => false,
    eventName: MenubarEvents.VISIBILITY_CHANGED,
    staleTime: Infinity,
  });

  useDisableRightClick();

  return { menuHidden };
};
