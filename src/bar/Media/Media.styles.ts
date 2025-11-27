import { css } from '@linaria/core';

import { LAPTOP_MEDIA_QUERY } from '@/utils/media-query';

const laptopMediaQuery = `@media ${LAPTOP_MEDIA_QUERY}`;

export const media = css`
  position: fixed;
  top: 0;
  bottom: 0;
  left: 50%;
  transform: translateX(-50%);

  display: grid;
  grid-auto-flow: column;
  row-gap: 4px;

  max-width: 480px;
  height: 100%;
  padding-left: 1px;

  ${laptopMediaQuery} {
    max-width: 300px;
  }
`;

export const label = css`
  overflow: hidden;

  white-space: nowrap;
`;

export const artwork = css`
  overflow: hidden;

  width: 24px;
  height: 24px;
  border-radius: 10px;

  object-fit: cover;
`;
