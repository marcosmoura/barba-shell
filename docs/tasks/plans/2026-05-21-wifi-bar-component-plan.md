# WiFi Bar Component Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a WiFi status bar component showing connection status/network name that opens System Settings Wi-Fi pane on click.

**Architecture:** Rust native component parses macOS `networksetup` output, exposes via Tauri command. React component polls via React Query, renders icon + label.

**Tech Stack:** Rust (Tauri command, shell plugin), TypeScript/React (React Query, Linaria, hugeicons)

---

## Task 1: Rust WiFi component (`wifi.rs`)

**Files:**

- Create: `app/native/src/modules/bar/components/wifi.rs`
- Modify: `app/native/src/modules/bar/components/mod.rs` (add `pub mod wifi;`)
- Modify: `app/native/src/lib.rs` (register command)
- Modify: `app/native/src/modules/bar/components/apps.rs` (add "Wi-Fi" URL entry)

- [ ] **Step 1: Create wifi.rs with WifiInfo types, parsing, and command**

````rust
use serde::Serialize;
use tauri_plugin_shell::ShellExt;

use crate::error::StacheError;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WifiStatus {
    Unknown,
    Off,
    Disconnected,
    Connected,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WifiInfo {
    pub status: WifiStatus,
    pub network_name: Option<String>,
}

pub(crate) fn parse_airport_power(output: &str) -> WifiStatus {
    let trimmed = output.trim();
    if trimmed.contains("Off") {
        WifiStatus::Off
    } else if trimmed.contains("On") {
        // Power is on, but we don't know if connected yet
        WifiStatus::Disconnected
    } else {
        WifiStatus::Unknown
    }
}

pub(crate) fn parse_airport_network(output: &str) -> Option<String> {
    let trimmed = output.trim();
    if trimmed.contains("Error") || trimmed.contains("not associated") || trimmed.is_empty() {
        return None;
    }
    // Format: "Current Wi-Fi Network: MyNetwork"
    trimmed
        .strip_prefix("Current Wi-Fi Network: ")
        .map(|s| s.trim().to_string())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn get_wifi_info(app: tauri::AppHandle) -> WifiInfo {
    let power_output = run_networksetup_command(&app, "-getairportpower", "en0");
    let status = power_output
        .as_ref()
        .map(|o| parse_airport_power(o))
        .unwrap_or(WifiStatus::Unknown);

    if status == WifiStatus::Off {
        return WifiInfo {
            status: WifiStatus::Off,
            network_name: None,
        };
    }

    let network_output = run_networksetup_command(&app, "-getairportnetwork", "en0");
    match network_output {
        Some(ref output) => {
            let network_name = parse_airport_network(output);
            if network_name.is_some() {
                WifiInfo {
                    status: WifiStatus::Connected,
                    network_name,
                }
            } else {
                WifiInfo {
                    status: WifiStatus::Disconnected,
                    network_name: None,
                }
            }
        }
        None => WifiInfo {
            status: WifiStatus::Unknown,
            network_name: None,
        },
    }
}

fn run_networksetup_command(app: &tauri::AppHandle, flag: &str, interface: &str) -> Option<String> {
    let shell = app.shell();
    let output = tauri::async_runtime::block_on(async {
        shell
            .command("networksetup")
            .args([flag, interface])
            .output()
            .await
    });

    match output {
        Ok(o) if o.status.success() => {
            String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
        }
        _ => None,
    }
}

- [ ] **Step 2: Run test to verify compiles**

Run: `cargo check -p stache 2>&1 | head -50`

- [ ] **Step 3: Add unit tests for parsing functions**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_airport_power_on() {
        assert_eq!(parse_airport_power("Wi-Fi Power (en0): On\n"), WifiStatus::Disconnected);
    }

    #[test]
    fn parse_airport_power_off() {
        assert_eq!(parse_airport_power("Wi-Fi Power (en0): Off\n"), WifiStatus::Off);
    }

    #[test]
    fn parse_airport_power_garbage() {
        assert_eq!(parse_airport_power("unexpected output"), WifiStatus::Unknown);
    }

    #[test]
    fn parse_airport_network_connected() {
        let result = parse_airport_network("Current Wi-Fi Network: MyHomeWiFi\n");
        assert_eq!(result, Some("MyHomeWiFi".to_string()));
    }

    #[test]
    fn parse_airport_network_error() {
        assert_eq!(parse_airport_network("Error: no such interface"), None);
    }

    #[test]
    fn parse_airport_network_not_associated() {
        assert_eq!(parse_airport_network("not associated"), None);
    }

    #[test]
    fn parse_airport_network_empty() {
        assert_eq!(parse_airport_network(""), None);
    }

    #[test]
    fn parse_airport_network_ssid_with_spaces() {
        let result = parse_airport_network("Current Wi-Fi Network: My Home Network\n");
        assert_eq!(result, Some("My Home Network".to_string()));
    }
}
````

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p stache -- wifi 2>&1`

- [ ] **Step 5: Add `pub mod wifi;` to `mod.rs`**

Edit `app/native/src/modules/bar/components/mod.rs`:

```diff
 pub mod tiling;
+pub mod wifi;
 pub mod weather;
```

- [ ] **Step 6: Register `get_wifi_info` command in `lib.rs`**

Edit `app/native/src/lib.rs` in `generate_handler![]`:

```diff
+            bar::components::wifi::get_wifi_info,
```

- [ ] **Step 7: Add "Wi-Fi" URL entry to `apps.rs` `ALLOWED_APPS`**

```diff
- const ALLOWED_APPS: [AppEntry; 7] = [
+ const ALLOWED_APPS: [AppEntry; 8] = [
     AppEntry::app("Activity Monitor"),
     AppEntry::app("Clock"),
     AppEntry::app("Microsoft Edge Dev"),
     AppEntry::app("Spotify"),
     AppEntry::app("Tidal"),
     AppEntry::app("Weather"),
+    AppEntry::url(
+        "Wi-Fi",
+        "x-apple.systempreferences:com.apple.wifi",
+    ),
     AppEntry::url(
         "Battery",
         "x-apple.systempreferences:com.apple.Battery-Settings.extension",
     ),
 ];
```

- [ ] **Step 8: Run full native lint**

Run: `cargo clippy --workspace --all -- -D warnings 2>&1 | tail -20`

- [ ] **Step 9: Run all native tests**

Run: `cargo nextest run --workspace 2>&1 | tail -20`

---

## Task 2: React WiFi component

**Files:**

- Create: `app/ui/renderer/bar/Status/Wifi/Wifi.types.ts`
- Create: `app/ui/renderer/bar/Status/Wifi/Wifi.state.ts`
- Create: `app/ui/renderer/bar/Status/Wifi/Wifi.tsx`
- Create: `app/ui/renderer/bar/Status/Wifi/Wifi.test.tsx`
- Create: `app/ui/renderer/bar/Status/Wifi/index.ts`
- Modify: `app/ui/renderer/bar/Status/Status.tsx`

- [ ] **Step 1: Create `Wifi.types.ts`**

```typescript
export type WifiStatus = 'Unknown' | 'Off' | 'Disconnected' | 'Connected';

export type WifiInfo = {
  status: WifiStatus;
  networkName: string | null;
};
```

- [ ] **Step 2: Create `Wifi.state.ts`**

```typescript
import { useCallback, useMemo } from 'react';

import { WifiIcon, WifiOff01Icon } from '@hugeicons/core-free-icons';
import { useSuspenseQuery } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';

import { colors } from '@/design-system';

import type { WifiInfo, WifiStatus } from './Wifi.types';

const fetchWifi = (): Promise<WifiInfo> => invoke<WifiInfo>('get_wifi_info');

const STATUS_LABELS: Record<WifiStatus, string | null> = {
  Unknown: null,
  Off: 'Wi-Fi Off',
  Disconnected: 'Disconnected',
  Connected: null, // use network name
};

export const useWifi = () => {
  const { data: wifi } = useSuspenseQuery({
    queryKey: ['wifi'],
    queryFn: fetchWifi,
    refetchInterval: 5000,
    refetchOnMount: true,
  });

  const status = wifi?.status ?? 'Unknown';
  const networkName = wifi?.networkName ?? null;

  const label = useMemo((): string | null => {
    if (status === 'Connected' && networkName) {
      return networkName;
    }
    return STATUS_LABELS[status] ?? null;
  }, [status, networkName]);

  const icon = useMemo(() => {
    return status === 'Off' || status === 'Disconnected' ? WifiOff01Icon : WifiIcon;
  }, [status]);

  const color = useMemo(() => {
    if (status === 'Connected') {
      return colors.text;
    }
    return colors.subtext0;
  }, [status]);

  const onClick = useCallback(() => invoke('open_app', { name: 'Wi-Fi' }), []);

  return { status, label, icon, color, onClick };
};
```

- [ ] **Step 3: Create `Wifi.tsx`**

```tsx
import { Button } from '@/components/Button';
import { Icon } from '@/components/Icon';
import { Surface } from '@/components/Surface';

import { useWifi } from './Wifi.state';

export const Wifi = () => {
  const { status, label, icon, color, onClick } = useWifi();

  if (status === 'Unknown') {
    return null;
  }

  return (
    <Surface as={Button} onClick={onClick}>
      <Icon icon={icon} color={color} />
      {label && <span>{label}</span>}
    </Surface>
  );
};
```

- [ ] **Step 4: Create `index.ts`**

```typescript
export { Wifi } from './Wifi';
```

- [ ] **Step 5: Create `Wifi.test.tsx`**

```typescript
import { describe, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';

import { createQueryClientWrapper, createTestQueryClient } from '@/tests/utils';

import { Wifi } from './Wifi';

describe('Wifi Component', () => {
  test('renders wifi info when connected', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['wifi'], {
      status: 'Connected',
      networkName: 'HomeWiFi',
    });

    const { getByText } = await render(<Wifi />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(getByText('HomeWiFi')).toBeDefined();
    });

    queryClient.clear();
  });

  test('renders wifi off label', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['wifi'], {
      status: 'Off',
      networkName: null,
    });

    const { getByText } = await render(<Wifi />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(getByText('Wi-Fi Off')).toBeDefined();
    });

    queryClient.clear();
  });

  test('renders disconnected label', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['wifi'], {
      status: 'Disconnected',
      networkName: null,
    });

    const { getByText } = await render(<Wifi />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(getByText('Disconnected')).toBeDefined();
    });

    queryClient.clear();
  });

  test('renders nothing when unknown', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['wifi'], {
      status: 'Unknown',
      networkName: null,
    });

    const { container } = await render(<Wifi />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(container.querySelector('button')).toBeNull();
    });

    queryClient.clear();
  });

  test('renders icon when wifi is off', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['wifi'], {
      status: 'Off',
      networkName: null,
    });

    const { container } = await render(<Wifi />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(container.querySelector('svg')).toBeDefined();
    });

    queryClient.clear();
  });

  test('renders icon when connected', async () => {
    const queryClient = createTestQueryClient();
    queryClient.setQueryData(['wifi'], {
      status: 'Connected',
      networkName: 'OfficeNet',
    });

    const { container } = await render(<Wifi />, {
      wrapper: createQueryClientWrapper(queryClient),
    });

    await vi.waitFor(() => {
      expect(container.querySelector('svg')).toBeDefined();
    });

    queryClient.clear();
  });
});
```

- [ ] **Step 6: Update `Status.tsx` to include WiFi**

```diff
 import { Battery } from './Battery';
 import { Clock } from './Clock';
 import { Cpu } from './Cpu';
 import { KeepAwake } from './KeepAwake';
 import { Weather } from './Weather';
+import { Wifi } from './Wifi';

 export const Status = () => {
   return (
     <Stack data-testid="status-container">
       <Weather />
       <KeepAwake />
+      <Wifi />
       <Cpu />
       <Battery />
       <Clock />
     </Stack>
   );
 };
```

- [ ] **Step 7: Add mock to `app/ui/tests/setup.ts`**

```diff
   get_cpu_info: { usage: 25, temperature: 50 },
+  get_wifi_info: { status: 'Connected', networkName: 'TestWiFi' },
```

- [ ] **Step 8: Run UI tests**

Run: `pnpm test:ui 2>&1 | tail -40`

- [ ] **Step 9: Run full lint**

Run: `pnpm lint 2>&1 | tail -40`
