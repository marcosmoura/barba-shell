use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{CapsKey, KEY_CAPS_LOCK, SYNTHETIC_CAPS_EVENT_ALLOWANCE_MILLIS};
use crate::config::ShortcutCommands;

#[derive(Debug, Default)]
pub(super) struct CapsState {
    mode: CapsMode,
    active_key: Option<CapsKey>,
    stable_caps_on: bool,
    press_started_caps_on: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum CapsMode {
    #[default]
    Idle,
    CapsHeld,
    ChordUsed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CapsInput {
    CapsDown,
    CapsUp,
    KeyDown(CapsKey, bool),
    KeyUp(CapsKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CapsDecision {
    Pass,
    Suppress,
    Execute(CapsKey),
    EnsureCapsState(bool),
}

#[derive(Debug, Default)]
pub(super) struct CapsAction {
    pub suppress: bool,
    pub ensure_caps_on: Option<bool>,
    pub commands: Option<ShortcutCommands>,
}

#[derive(Debug, Default)]
pub(super) struct SyntheticCapsEventAllowance {
    remaining: u8,
    expires_at: Option<Instant>,
}

pub(super) static STATE: Mutex<CapsState> = Mutex::new(CapsState {
    mode: CapsMode::Idle,
    active_key: None,
    stable_caps_on: false,
    press_started_caps_on: false,
});

pub(super) static SYNTHETIC_CAPS_EVENTS: Mutex<SyntheticCapsEventAllowance> =
    Mutex::new(SyntheticCapsEventAllowance { remaining: 0, expires_at: None });

impl CapsState {
    pub(super) fn set_caps_lock_state(&mut self, caps_on: bool) {
        self.stable_caps_on = caps_on;
        if self.mode == CapsMode::Idle {
            self.press_started_caps_on = caps_on;
        }
    }

    pub(super) fn handle_input(
        &mut self,
        input: CapsInput,
        has_binding: impl Fn(CapsKey) -> bool,
    ) -> CapsDecision {
        match input {
            CapsInput::CapsDown => {
                if self.mode == CapsMode::Idle {
                    self.mode = CapsMode::CapsHeld;
                    self.active_key = None;
                    self.press_started_caps_on = self.stable_caps_on;
                }
                CapsDecision::Pass
            }
            CapsInput::CapsUp => match self.mode {
                CapsMode::CapsHeld => {
                    let target_on = !self.press_started_caps_on;
                    self.mode = CapsMode::Idle;
                    self.active_key = None;
                    self.stable_caps_on = target_on;
                    CapsDecision::EnsureCapsState(target_on)
                }
                CapsMode::ChordUsed => {
                    let target_on = self.press_started_caps_on;
                    self.mode = CapsMode::Idle;
                    self.stable_caps_on = target_on;
                    CapsDecision::EnsureCapsState(target_on)
                }
                CapsMode::Idle => CapsDecision::Pass,
            },
            CapsInput::KeyDown(key, _) if key == CapsKey::new(KEY_CAPS_LOCK) => {
                CapsDecision::Suppress
            }
            CapsInput::KeyDown(key, true) if self.active_key == Some(key) => CapsDecision::Suppress,
            CapsInput::KeyDown(key, is_repeat) => match self.mode {
                CapsMode::CapsHeld
                    if has_binding(key) && !is_repeat && self.active_key.is_none() =>
                {
                    self.mode = CapsMode::ChordUsed;
                    self.active_key = Some(key);
                    CapsDecision::Execute(key)
                }
                CapsMode::CapsHeld if !is_repeat && self.active_key.is_none() => {
                    self.mode = CapsMode::ChordUsed;
                    CapsDecision::Pass
                }
                CapsMode::ChordUsed
                    if has_binding(key) && !is_repeat && self.active_key.is_none() =>
                {
                    self.active_key = Some(key);
                    CapsDecision::Execute(key)
                }
                CapsMode::ChordUsed if self.active_key == Some(key) => CapsDecision::Suppress,
                _ => CapsDecision::Pass,
            },
            CapsInput::KeyUp(key) => {
                if self.active_key == Some(key) {
                    self.active_key = None;
                    CapsDecision::Suppress
                } else {
                    CapsDecision::Pass
                }
            }
        }
    }
}

impl SyntheticCapsEventAllowance {
    pub(super) fn arm(&mut self, now: Instant, event_count: u8) {
        if event_count == 0 {
            self.clear();
            return;
        }

        self.remaining = event_count;
        self.expires_at = Some(now + Duration::from_millis(SYNTHETIC_CAPS_EVENT_ALLOWANCE_MILLIS));
    }

    pub(super) fn consume(&mut self, now: Instant) -> bool {
        let Some(expires_at) = self.expires_at else {
            return false;
        };

        if self.remaining == 0 || now > expires_at {
            self.clear();
            return false;
        }

        self.remaining -= 1;
        if self.remaining == 0 {
            self.expires_at = None;
        }

        true
    }

    const fn clear(&mut self) {
        self.remaining = 0;
        self.expires_at = None;
    }
}

pub(super) fn action_for_input(
    state: &mut CapsState,
    input: CapsInput,
    bindings: &std::collections::HashMap<CapsKey, super::CapsBinding>,
) -> CapsAction {
    let decision = state.handle_input(input, |key| bindings.contains_key(&key));

    match decision {
        CapsDecision::Pass => CapsAction::default(),
        CapsDecision::Suppress => CapsAction {
            suppress: true,
            ..CapsAction::default()
        },
        CapsDecision::EnsureCapsState(target_on) => CapsAction {
            suppress: false,
            ensure_caps_on: Some(target_on),
            commands: None,
        },
        CapsDecision::Execute(key) => CapsAction {
            suppress: true,
            ensure_caps_on: Some(state.press_started_caps_on),
            commands: bindings.get(&key).map(|binding| binding.commands.clone()),
        },
    }
}
