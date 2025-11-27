import { css } from '@linaria/core';

export const wrapper = css`
  position: relative;

  overflow: hidden;

  max-width: 100%;
`;

export const label = css`
  display: inline-block;

  white-space: nowrap;
`;

export const scrollingLabel = css`
  animation: scroll-text var(--scroll-duration, 5s) linear infinite alternate;

  @keyframes scroll-text {
    0%,
    20% {
      transform: translateX(0);
    }

    80%,
    100% {
      transform: translateX(var(--scroll-distance, 0px));
    }
  }
`;
