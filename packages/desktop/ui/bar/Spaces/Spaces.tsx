import {
  ComputerTerminal01Icon,
  SourceCodeIcon,
  AiBrowserIcon,
  SpotifyIcon,
  FigmaIcon,
  SlackIcon,
  DashboardCircleIcon,
  AppleFinderIcon,
  Mail02Icon,
  AppleReminderIcon,
  AppStoreIcon,
  BrowserIcon,
  Mail01Icon,
  UserMultiple02Icon,
  HardDriveIcon,
  SecurityPasswordIcon,
  SourceCodeCircleIcon,
  VisualStudioCodeIcon,
  DiscordIcon,
  WhatsappIcon,
  ZoomIcon,
} from '@hugeicons/core-free-icons';
import type { IconSvgElement } from '@hugeicons/react';
import { cx } from '@linaria/core';

import { Button } from '@/components/Button';
import { Icon } from '@/components/Icon';
import { Surface } from '@/components/Surface';

import { useSpaces } from './Spaces.state';
import * as styles from './Spaces.styles';

const workspaceIcons: Record<string, IconSvgElement> = {
  terminal: ComputerTerminal01Icon,
  coding: SourceCodeIcon,
  browser: AiBrowserIcon,
  music: SpotifyIcon,
  design: FigmaIcon,
  communication: SlackIcon,
  misc: DashboardCircleIcon,
  files: AppleFinderIcon,
  mail: Mail02Icon,
  tasks: AppleReminderIcon,
};

const appIcons = {
  'App Store': AppStoreIcon,
  'Microsoft Edge Dev': BrowserIcon,
  'Microsoft Outlook': Mail01Icon,
  'Microsoft Teams': UserMultiple02Icon,
  'Proton Drive': HardDriveIcon,
  'Proton Pass': SecurityPasswordIcon,
  'Zed Preview': SourceCodeCircleIcon,
  Code: VisualStudioCodeIcon,
  Discord: DiscordIcon,
  Figma: FigmaIcon,
  Finder: AppleFinderIcon,
  Ghostty: ComputerTerminal01Icon,
  Reminders: AppleReminderIcon,
  Slack: SlackIcon,
  Spotify: SpotifyIcon,
  WhatsApp: WhatsappIcon,
  // WTF? There is a special character in the app name
  '‎WhatsApp': WhatsappIcon,
  Zoom: ZoomIcon,
} as const;

const getAppIcon = (name: string) => {
  const appName = name.trim() as keyof typeof appIcons;

  return appIcons[appName] || DashboardCircleIcon;
};

export const Spaces = () => {
  const { workspaces, focusedApp, onSpaceClick } = useSpaces();

  if (!workspaces) {
    return null;
  }

  return (
    <div className={styles.spaces} data-test-id="spaces-container">
      <Surface className={styles.workspaces}>
        {workspaces.map(({ key, name, isFocused }) => (
          <Button
            key={key}
            className={cx(styles.workspace, isFocused && styles.workspaceActive)}
            active={isFocused}
            onClick={onSpaceClick(key)}
          >
            <Icon icon={workspaceIcons[key]} />
            {isFocused && <span>{name}</span>}
          </Button>
        ))}
      </Surface>

      {focusedApp && (
        <Surface className={styles.app}>
          <Icon icon={getAppIcon(focusedApp)} />
          <span>{focusedApp}</span>
        </Surface>
      )}
    </div>
  );
};
