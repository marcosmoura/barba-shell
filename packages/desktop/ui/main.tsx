import { StrictMode, Suspense, lazy } from 'react';
import { createRoot } from 'react-dom/client';

import './main.css';

import { getCurrentWindow } from '@tauri-apps/api/window';

const resolveModule = (moduleName: string) => (module: Record<string, React.ComponentType>) => ({
  default: module[moduleName],
});
const Bar = lazy(() => import('./renderer/bar').then(resolveModule('Bar')));
const Widgets = lazy(() => import('./renderer/widgets').then(resolveModule('Widgets')));

const windowName = getCurrentWindow().label;

console.log('Current window name:', windowName);

createRoot(document.getElementById('root') as HTMLElement).render(
  <StrictMode>
    <Suspense fallback={null}>
      {windowName === 'bar' && <Bar />}
      {windowName === 'widgets' && <Widgets />}
    </Suspense>
  </StrictMode>,
);
