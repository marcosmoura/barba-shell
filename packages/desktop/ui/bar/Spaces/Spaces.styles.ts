import { css } from '@linaria/core';

export const spaces = css`
  position: fixed;
  top: 0;
  bottom: 0;
  left: 0;

  display: grid;
  grid-auto-flow: column;
  column-gap: 4px;
  align-items: center;
`;

export const workspaces = css`
  display: grid;
  grid-auto-flow: column;
  align-items: center;
`;

export const workspace = css`
  padding: 0 8px;
`;

export const workspaceActive = css`
  padding: 0 10px;
`;

export const app = css`
  display: grid;
  grid-auto-flow: column;
  column-gap: 6px;
  align-items: center;

  height: 100%;
  padding: 0 10px;
`;
