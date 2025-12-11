//! Audio device management module.
//!
//! Automatically switches audio input and output devices based on connected hardware.
//! This module monitors for device changes and applies priority-based switching rules.
//!
//! # Device Priority (Output)
//! 1. `AirPods` (when connected)
//! 2. Current device if `AirPlay` (don't switch away from `AirPlay`)
//! 3. External Speakers (when audio interface connected)
//! 4. Microsoft Teams Audio (when in use and audio interface connected)
//! 5. `MacBook` Pro built-in speakers (fallback)
//!
//! # Device Priority (Input)
//! 1. External USB microphone (AT2020USB)
//! 2. `AirPods` microphone
//! 3. `MacBook` Pro built-in microphone (fallback)

use std::ffi::c_void;
use std::ptr::{NonNull, null};
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

/// Stores the Sender used by audio property listeners.
/// This is intentionally kept alive for the application's lifetime since the
/// `CoreAudio` property listeners need a valid pointer to send device change events.
/// The raw pointer is passed to `CoreAudio` callbacks and must remain valid.
static LISTENER_SENDER: OnceLock<Box<Sender<()>>> = OnceLock::new();

use coreaudio::audio_unit::Scope;
use coreaudio::audio_unit::macos_helpers::{
    get_audio_device_ids, get_audio_device_supports_scope, get_default_device_id, get_device_name,
};
use objc2_core_audio::{
    AudioDeviceID, AudioObjectAddPropertyListener, AudioObjectID, AudioObjectPropertyAddress,
    AudioObjectSetPropertyData, kAudioHardwareNoError, kAudioHardwarePropertyDefaultInputDevice,
    kAudioHardwarePropertyDefaultOutputDevice, kAudioHardwarePropertyDevices,
    kAudioObjectPropertyElementMain, kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject,
};

use crate::utils::thread::spawn_named_thread;

/// Represents an audio device with its ID and name.
#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub id: AudioDeviceID,
    pub name: String,
}

impl AudioDevice {
    /// Creates a new `AudioDevice` from a device ID.
    fn from_id(id: AudioDeviceID) -> Option<Self> {
        get_device_name(id).ok().map(|name| Self { id, name })
    }

    /// Checks if the device name contains the given substring (case-insensitive).
    fn name_contains(&self, substring: &str) -> bool {
        self.name.to_lowercase().contains(&substring.to_lowercase())
    }
}

/// Gets all output audio devices.
fn get_output_devices() -> Vec<AudioDevice> {
    get_audio_device_ids()
        .unwrap_or_default()
        .into_iter()
        .filter(|&id| get_audio_device_supports_scope(id, Scope::Output).unwrap_or(false))
        .filter_map(AudioDevice::from_id)
        .collect()
}

/// Gets all input audio devices.
fn get_input_devices() -> Vec<AudioDevice> {
    get_audio_device_ids()
        .unwrap_or_default()
        .into_iter()
        .filter(|&id| get_audio_device_supports_scope(id, Scope::Input).unwrap_or(false))
        .filter_map(AudioDevice::from_id)
        .collect()
}

/// Gets the current default output device.
fn get_default_output_device() -> Option<AudioDevice> {
    get_default_device_id(false).and_then(AudioDevice::from_id)
}

/// Gets the current default input device.
fn get_default_input_device() -> Option<AudioDevice> {
    get_default_device_id(true).and_then(AudioDevice::from_id)
}

/// Size of `AudioDeviceID` in bytes as u32.
#[allow(clippy::cast_possible_truncation)] // AudioDeviceID is u32, so size is always 4 bytes
const AUDIO_DEVICE_ID_SIZE: u32 = std::mem::size_of::<AudioDeviceID>() as u32;

/// Sets the default output device.
fn set_default_output_device(device_id: AudioDeviceID) -> bool {
    let property_address = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDefaultOutputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };

    let status = unsafe {
        AudioObjectSetPropertyData(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&property_address),
            0,
            null(),
            AUDIO_DEVICE_ID_SIZE,
            NonNull::from(&device_id).cast(),
        )
    };

    status == kAudioHardwareNoError
}

/// Sets the default input device.
fn set_default_input_device(device_id: AudioDeviceID) -> bool {
    let property_address = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDefaultInputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };

    let status = unsafe {
        AudioObjectSetPropertyData(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&property_address),
            0,
            null(),
            AUDIO_DEVICE_ID_SIZE,
            NonNull::from(&device_id).cast(),
        )
    };

    status == kAudioHardwareNoError
}

/// Finds a device by name (case-insensitive substring match).
fn find_device_by_name<'a>(devices: &'a [AudioDevice], name: &str) -> Option<&'a AudioDevice> {
    devices.iter().find(|device| device.name_contains(name))
}

/// Determines the target output device based on priority rules.
///
/// Priority:
/// 1. `AirPods` (when connected)
/// 2. Current device if `AirPlay` (don't switch away)
/// 3. External Speakers (when audio interface connected)
/// 4. Microsoft Teams Audio (when in use with audio interface)
/// 5. `MacBook` Pro built-in speakers
fn get_target_output_device<'a>(
    current: &AudioDevice,
    devices: &'a [AudioDevice],
) -> Option<&'a AudioDevice> {
    // Check for AirPods first (highest priority)
    if let Some(airpods) = find_device_by_name(devices, "airpods") {
        return Some(airpods);
    }

    // Don't switch away from AirPlay
    if current.name_contains("airplay") {
        return devices.iter().find(|d| d.id == current.id);
    }

    // Check for audio interface (MiniFuse)
    if find_device_by_name(devices, "minifuse").is_some()
        && let Some(speakers) = find_device_by_name(devices, "external speakers")
    {
        return Some(speakers);
    }

    // Check if Teams is available
    if let Some(teams) = find_device_by_name(devices, "microsoft teams audio") {
        return Some(teams);
    }

    // Fallback to MacBook Pro speakers
    find_device_by_name(devices, "MacBook Pro")
}

/// Determines the target input device based on priority rules.
///
/// Priority:
/// 1. External USB microphone (AT2020USB)
/// 2. `AirPods` microphone
/// 3. `MacBook` Pro built-in microphone
fn get_target_input_device<'a>(
    current: &AudioDevice,
    devices: &'a [AudioDevice],
) -> Option<&'a AudioDevice> {
    // External USB microphone has highest priority
    if let Some(mic) = find_device_by_name(devices, "at2020usb") {
        return Some(mic);
    }

    // AirPods microphone
    if let Some(airpods) = find_device_by_name(devices, "airpods") {
        return Some(airpods);
    }

    // Fallback to MacBook Pro microphone
    if let Some(macbook) = find_device_by_name(devices, "MacBook Pro") {
        return Some(macbook);
    }

    // Keep current if nothing else matches
    devices.iter().find(|d| d.id == current.id)
}

/// Handles output device changes by applying priority rules.
fn handle_output_device_change() {
    let Some(current) = get_default_output_device() else {
        return;
    };

    let devices = get_output_devices();
    let Some(target) = get_target_output_device(&current, &devices) else {
        return;
    };

    if current.id == target.id {
        return;
    }

    if set_default_output_device(target.id) {
        println!("Default output device set to {}", target.name);
    } else {
        eprintln!("Failed to set default output device to {}", target.name);
    }
}

/// Handles input device changes by applying priority rules.
fn handle_input_device_change() {
    let Some(current) = get_default_input_device() else {
        return;
    };

    let devices = get_input_devices();
    let Some(target) = get_target_input_device(&current, &devices) else {
        return;
    };

    if current.id == target.id {
        return;
    }

    if set_default_input_device(target.id) {
        println!("Default input device set to {}", target.name);
    } else {
        eprintln!("Failed to set default input device to {}", target.name);
    }
}

/// Handles all audio device changes.
fn on_audio_device_change() {
    handle_output_device_change();
    handle_input_device_change();
}

/// Property listener callback for audio device changes.
///
/// # Safety
/// This function is called by `CoreAudio` and expects valid pointers.
unsafe extern "C-unwind" fn audio_device_property_listener(
    _in_object_id: AudioObjectID,
    _in_number_addresses: u32,
    _in_addresses: NonNull<AudioObjectPropertyAddress>,
    in_client_data: *mut c_void,
) -> i32 {
    if !in_client_data.is_null() {
        // SAFETY: We know in_client_data is a valid Sender pointer from init_audio_device_watcher
        let tx = unsafe { &*in_client_data.cast::<Sender<()>>() };
        let _ = tx.send(());
    }
    0 // kAudioHardwareNoError
}

/// Registers listeners for audio device changes.
///
/// The `Sender` is stored in a static to ensure it lives for the application's
/// lifetime, as `CoreAudio` callbacks require a valid pointer.
fn register_audio_listeners(tx: Sender<()>) {
    // Store the sender in a static to ensure it lives for the app's lifetime.
    // CoreAudio callbacks will use this pointer to send device change events.
    let sender_box = LISTENER_SENDER.get_or_init(|| Box::new(tx));
    // Cast to *mut for CoreAudio API compatibility (the callback only reads from it)
    let tx_ptr: *mut c_void =
        std::ptr::from_ref::<Sender<()>>(sender_box.as_ref()).cast_mut().cast();

    // Listen for default output device changes
    let output_property_address = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDefaultOutputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };

    unsafe {
        AudioObjectAddPropertyListener(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&output_property_address),
            Some(audio_device_property_listener),
            tx_ptr,
        );
    }

    // Listen for default input device changes
    let input_property_address = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDefaultInputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };

    unsafe {
        AudioObjectAddPropertyListener(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&input_property_address),
            Some(audio_device_property_listener),
            tx_ptr,
        );
    }

    // Listen for device list changes (connect/disconnect)
    let devices_property_address = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDevices,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };

    unsafe {
        AudioObjectAddPropertyListener(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&devices_property_address),
            Some(audio_device_property_listener),
            tx_ptr,
        );
    }
}

static AUDIO_WATCHER_ONCE: OnceLock<()> = OnceLock::new();

/// Initializes the audio device watcher.
///
/// This function spawns a background thread that monitors for audio device
/// changes and automatically switches devices based on priority rules.
fn init_audio_device_watcher() {
    spawn_named_thread("audio-device-watcher", move || {
        let (tx, rx) = channel();

        // Register all audio device listeners
        register_audio_listeners(tx);

        // Wait for device change events
        while rx.recv().is_ok() {
            on_audio_device_change();
        }
    });
}

/// Initializes the audio module.
///
/// Sets up device watchers and applies initial device configuration.
pub fn init() {
    if AUDIO_WATCHER_ONCE.set(()).is_err() {
        return;
    }

    // Apply initial device configuration
    on_audio_device_change();

    // Start watching for device changes
    init_audio_device_watcher();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_device_name_contains_case_insensitive() {
        let device = AudioDevice {
            id: 1,
            name: "AirPods Pro".to_string(),
        };

        assert!(device.name_contains("airpods"));
        assert!(device.name_contains("AIRPODS"));
        assert!(device.name_contains("AirPods"));
        assert!(!device.name_contains("macbook"));
    }

    #[test]
    fn find_device_by_name_returns_matching_device() {
        let devices = vec![
            AudioDevice {
                id: 1,
                name: "MacBook Pro Speakers".to_string(),
            },
            AudioDevice {
                id: 2,
                name: "AirPods Pro".to_string(),
            },
            AudioDevice {
                id: 3,
                name: "External Speakers".to_string(),
            },
        ];

        let found = find_device_by_name(&devices, "airpods");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, 2);

        let not_found = find_device_by_name(&devices, "minifuse");
        assert!(not_found.is_none());
    }

    #[test]
    fn get_target_output_device_prioritizes_airpods() {
        let current = AudioDevice {
            id: 1,
            name: "MacBook Pro Speakers".to_string(),
        };

        let devices = vec![
            AudioDevice {
                id: 1,
                name: "MacBook Pro Speakers".to_string(),
            },
            AudioDevice {
                id: 2,
                name: "AirPods Pro".to_string(),
            },
        ];

        let target = get_target_output_device(&current, &devices);
        assert!(target.is_some());
        assert_eq!(target.unwrap().id, 2);
    }

    #[test]
    fn get_target_output_device_keeps_airplay() {
        let current = AudioDevice {
            id: 3,
            name: "AirPlay Device".to_string(),
        };

        let devices = vec![
            AudioDevice {
                id: 1,
                name: "MacBook Pro Speakers".to_string(),
            },
            AudioDevice {
                id: 2,
                name: "External Speakers".to_string(),
            },
            AudioDevice {
                id: 3,
                name: "AirPlay Device".to_string(),
            },
        ];

        let target = get_target_output_device(&current, &devices);
        assert!(target.is_some());
        assert_eq!(target.unwrap().id, 3); // Should keep AirPlay
    }

    #[test]
    fn get_target_input_device_prioritizes_external_mic() {
        let current = AudioDevice {
            id: 1,
            name: "MacBook Pro Microphone".to_string(),
        };

        let devices = vec![
            AudioDevice {
                id: 1,
                name: "MacBook Pro Microphone".to_string(),
            },
            AudioDevice {
                id: 2,
                name: "AT2020USB+".to_string(),
            },
        ];

        let target = get_target_input_device(&current, &devices);
        assert!(target.is_some());
        assert_eq!(target.unwrap().id, 2);
    }

    #[test]
    fn get_target_input_device_falls_back_to_macbook() {
        let current = AudioDevice {
            id: 1,
            name: "MacBook Pro Microphone".to_string(),
        };

        let devices = vec![AudioDevice {
            id: 1,
            name: "MacBook Pro Microphone".to_string(),
        }];

        let target = get_target_input_device(&current, &devices);
        assert!(target.is_some());
        assert_eq!(target.unwrap().id, 1);
    }
}
