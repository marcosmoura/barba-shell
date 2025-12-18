import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { HugeiconsIcon } from '@hugeicons/react';

import type { FontAwesomeProps, HugeIconsProps, IconProps } from './Icon.types';

export const Icon = ({ pack, ...props }: IconProps) => {
  if (pack === 'fontawesome') {
    return <FontAwesomeIcon {...(props as FontAwesomeProps)} />;
  }

  const { icon, size = 18, strokeWidth = 1.8, ...rest } = props as HugeIconsProps;
  return <HugeiconsIcon icon={icon} size={size} strokeWidth={strokeWidth} {...rest} />;
};
