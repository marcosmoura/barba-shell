# Status Bar 07 — Weather Data

> Status: ✅ Normative

## Purpose

Weather exposes bar weather configuration to the frontend and projects the shared weather-store state into a clickable status item and widget content.

## Scope

- Weather configuration command and bar item presentation.
- The bar/widget's shared weather-store boundary.

### Out of Scope

| Excluded concern                          | Owner                                                                              | Boundary note                               |
| ----------------------------------------- | ---------------------------------------------------------------------------------- | ------------------------------------------- |
| Overlay open/close/placement              | [widgets/01 overlay lifecycle](../widgets/01-overlay-lifecycle.md)                 | Weather only requests its toggle.           |
| Application launch authorization          | [foundation/10 application shell](../foundation/10-application-shell.md)           | Widget launch is a consumer boundary.       |
| Config loading and active-file precedence | [foundation/01 configuration contract](../foundation/01-configuration-contract.md) | This spec owns weather fields, not parsing. |
| Bar composition                           | [bar/01 bar window lifecycle](01-bar-window-lifecycle.md)                          | Status owns placement.                      |

## Terminology

- **Configured** — `useWeatherStore().isConfigured` is true.
- **Current conditions** — the normalized weather-store snapshot used by the item.

## Data Contract

`get_weather_config` returns `{ provider: 'auto' | 'visual-crossing' | 'open-meteo', visualCrossingApiKey: string, defaultLocation: string }` with camel-case serialization. `WeatherProvider::Auto` selects Visual Crossing when a key is available and Open-Meteo otherwise; explicit Visual Crossing requires a key.

When configured, the bar renders its weather icon and `ceil(currentConditions.feelslike || 0)°C`; loading or absent conditions show `Loading...`. When not configured it renders nothing. Clicking toggles the Weather widget with the element's rectangle.

## Configuration Contract

`bar.weather.provider` defaults to `auto`; `apiKeys` and `defaultLocation` default to empty strings. The native command resolves the API-key file relative to the active configuration directory.

## Inputs

- Immutable weather configuration and its key-file path.
- Weather-store configuration/loading/current-condition state.
- Weather item click.

## State Transitions

| From               | Input                      | To              | Effect                                         |
| ------------------ | -------------------------- | --------------- | ---------------------------------------------- |
| Unconfigured       | store remains unconfigured | Hidden          | Render nothing.                                |
| Configured/loading | absent current conditions  | Visible loading | Render icon and `Loading...`.                  |
| Configured         | current conditions         | Visible data    | Render icon and rounded-up feels-like Celsius. |
| Visible            | click                      | Visible         | Toggle the Weather widget.                     |

## Outputs

The Tauri command returns the configuration payload without a `Result` error channel. The bar publishes no weather event; it requests a widget toggle.

## Derived Effects

The native command loads configured API keys from the resolved env file. Store/network/provider lifecycle is carried by `app/ui/stores/WeatherStore`; current bar code does not implement its own provider retry or refresh loop.

## Failure & Recovery

A missing/unusable key results in the returned empty key string rather than a command error. The bar hides when the store reports unconfigured and presents loading when configured data is absent. Store error/retry policy is outside this item's rendered contract.

## Cross-Module Contracts

[foundation/01](../foundation/01-configuration-contract.md) supplies active-config resolution. [widgets/01](../widgets/01-overlay-lifecycle.md) owns the toggle outcome. [bar/01](01-bar-window-lifecycle.md) owns status order.

## Acceptance Scenarios

1. Given default weather config, when retrieved, then provider is `auto` and text fields are empty.
2. Given an API-key path relative to the active config, when queried, then it is resolved from that directory.
3. Given unconfigured store state, when rendering, then the item is absent.
4. Given configured loading state, when rendering, then the label is `Loading...`.
5. Given feels-like 12.1, when rendering, then the label is `13°C`.
6. Given a click, when the item is present, then it toggles Weather rather than launching a separate bar app.
7. Given provider/store failure, when no current condition exists, then this item does not invent a retry policy.

## Testing Seam

`WeatherConfigInfo::from_config` and the bar-state label derivation are stable seams. Existing tests: `app/native/src/modules/bar/components/weather.rs:65-140`, `app/ui/renderer/bar/Status/Weather/Weather.test.tsx`, and `app/ui/stores/WeatherStore/WeatherStore.test.tsx`.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                        | Implementation evidence                                                                             | Test evidence                                                                                                                                                                          | Intended documentation                     | Disposition |
| ------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------ | ----------- |
| Config payload/defaults and env resolution              | `app/native/src/config/types/bar.rs:8-81`; `app/native/src/modules/bar/components/weather.rs:15-63` | `weather.rs` — `test_weather_config_info_default`; `test_weather_config_info_with_env_file`; `test_weather_config_info_missing_env_file`; `test_weather_config_info_absolute_env_path` | `status-bar.md:82-103`                     | Aligned     |
| Bar visibility, loading label, Celsius and widget click | `app/ui/renderer/bar/Status/Weather/Weather.state.ts:10-30`; `Weather.tsx:11-24`                    | `Weather.test.tsx` — `renders weather info`; `renders temperature label`; `renders loading state when no weather data`; `handles null feelslike value`                                 | `status-bar.md:82-103`; `widgets.md:51-63` | Aligned     |
