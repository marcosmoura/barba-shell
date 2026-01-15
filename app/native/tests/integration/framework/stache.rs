//! Stache process management for integration tests.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::{fs, thread};

use tempfile::NamedTempFile;

/// The log message that indicates stache tiling is fully initialized.
const READY_MESSAGE: &str = "stache: tiling: initialization complete";

/// Default timeout for stache to become ready.
const DEFAULT_READY_TIMEOUT: Duration = Duration::from_secs(30);

/// Apps to ignore during integration tests (by name).
/// These are common apps that may be running and would interfere with tests.
const APPS_TO_IGNORE: &[&str] = &[
    "1Password",
    "Activity Monitor",
    "Alfred",
    "Arc",
    "Bear",
    "Bitwarden",
    "Calendar",
    "CleanShot X",
    "Code",
    "Console",
    "Craft",
    "Cursor",
    "Cyberduck",
    "Dia",
    "Discord",
    "Docker Desktop",
    "Fantastical",
    "Figma",
    "Finder",
    "Firefox",
    "Fork",
    "Ghostty",
    "Google Chrome",
    "Insomnia",
    "iTerm2",
    "Kap",
    "Keychain Access",
    "Linear",
    "Logseq",
    "Loom",
    "Mail",
    "Messages",
    "Microsoft Edge Dev",
    "Microsoft Edge",
    "Microsoft Outlook",
    "Microsoft Teams (work or school)",
    "Microsoft Teams",
    "Music",
    "Notes",
    "Notion",
    "OBS",
    "Obsidian",
    "Postico",
    "Postman",
    "Preview",
    "Proton Pass",
    "Raycast",
    "Reminders",
    "Safari",
    "Shottr",
    "Signal",
    "Simulator",
    "Slack",
    "SourceTree",
    "Spotify",
    "Spotlight",
    "System Preferences",
    "System Settings",
    "TablePlus",
    "Telegram",
    "Things",
    "TIDAL",
    "Todoist",
    "Tower",
    "Transmit",
    "WhatsApp",
    "Xcode",
    "Zed",
    "Zed Preview",
    "Zoom",
];

/// Bundle IDs to always ignore during integration tests.
/// These catch apps even if their display name changes.
const BUNDLE_IDS_TO_IGNORE: &[&str] = &[
    // Apple system apps
    "com.apple.dock",
    "com.apple.finder",
    "com.apple.loginwindow",
    "com.apple.notificationcenterui",
    "com.apple.reminders",
    "com.apple.Spotlight",
    "com.apple.systempreferences",
    "com.apple.weather.menu",
    // Microsoft
    "com.microsoft.AzureVpnMac",
    "com.microsoft.edgemac",
    "com.microsoft.edgemac.Dev",
    "com.microsoft.Outlook",
    "com.microsoft.teams2",
    "com.microsoft.VSCode",
    // Browsers
    "company.thebrowser.dia",
    // Communication
    "com.hnc.Discord",
    "net.whatsapp.WhatsApp",
    // Development
    "com.mitchellh.ghostty",
    "dev.zed.Zed",
    "dev.zed.Zed-Preview",
    // Media & Creative
    "com.figma.Desktop",
    "com.NeuralDSP.ArchetypeGojiraX",
    "com.NeuralDSP.ArchetypeJohnMayerX",
    "com.NeuralDSP.ArchetypeNollyX",
    "com.NeuralDSP.FortinNamelessSuiteX",
    "com.spotify.client",
    "com.tidal.desktop",
    // Utilities
    "cc.ffitch.shottr",
    "com.raycast.macos",
    "com.wulkano.kap",
    "me.proton.pass.electron",
];

/// Manages a stache process for testing.
pub struct StacheProcess {
    /// The child process.
    child: Option<Child>,
    /// Flag indicating if the process is ready.
    ready: Arc<AtomicBool>,
    /// The temp config file (kept alive for the duration of the test).
    _config_file: NamedTempFile,
    /// The original fixture name used.
    fixture_name: String,
}

impl StacheProcess {
    /// Gets the path to the stache binary.
    pub fn binary_path() -> PathBuf {
        // In tests, the binary is in target/debug or target/release
        let mut path = std::env::current_exe()
            .expect("Failed to get current exe path")
            .parent()
            .expect("Failed to get parent directory")
            .parent()
            .expect("Failed to get grandparent directory")
            .to_path_buf();

        // Navigate from deps directory to the actual binary
        // Current exe is in target/debug/deps/integration-xxx
        // Binary is in target/debug/stache
        path.push("stache");

        if !path.exists() {
            // Try without the deps navigation (for different test runners)
            path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("target")
                .join("debug")
                .join("stache");
        }

        if !path.exists() {
            panic!(
                "Stache binary not found at {:?}. Make sure to run `cargo build -p stache` first.",
                path
            );
        }

        path
    }

    /// Gets the path to a fixture config file.
    fn fixture_path(fixture_name: &str) -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let fixture_path = manifest_dir
            .join("tests")
            .join("fixtures")
            .join(format!("{}.jsonc", fixture_name));

        if !fixture_path.exists() {
            panic!("Fixture not found: {:?}", fixture_path);
        }

        fixture_path
    }

    /// Loads a fixture file, merges in the ignore rules, and writes to a temp file.
    fn prepare_config(fixture_name: &str) -> NamedTempFile {
        let fixture_path = Self::fixture_path(fixture_name);
        let fixture_content = fs::read_to_string(&fixture_path)
            .unwrap_or_else(|e| panic!("Failed to read fixture {:?}: {}", fixture_path, e));

        // Strip comments from JSONC
        let stripped = json_comments::StripComments::new(fixture_content.as_bytes());
        let mut config: serde_json::Value = serde_json::from_reader(stripped)
            .unwrap_or_else(|e| panic!("Failed to parse fixture {:?}: {}", fixture_path, e));

        // Ensure tiling object exists
        let tiling = config
            .as_object_mut()
            .expect("Config must be an object")
            .entry("tiling")
            .or_insert_with(|| serde_json::json!({}));

        let tiling_obj = tiling.as_object_mut().expect("tiling must be an object");

        // Build ignore rules array
        let mut ignore_rules: Vec<serde_json::Value> = Vec::new();

        // Add app name rules (WindowRule uses camelCase: "appName")
        for app_name in APPS_TO_IGNORE {
            ignore_rules.push(serde_json::json!({
                "appName": app_name
            }));
        }

        // Add bundle ID rules (WindowRule uses camelCase: "appId")
        for bundle_id in BUNDLE_IDS_TO_IGNORE {
            ignore_rules.push(serde_json::json!({
                "appId": bundle_id
            }));
        }

        // Merge with existing ignore rules if any
        let total_rules = if let Some(existing) = tiling_obj.get_mut("ignore") {
            if let Some(existing_arr) = existing.as_array_mut() {
                let count = existing_arr.len() + ignore_rules.len();
                existing_arr.extend(ignore_rules);
                count
            } else {
                0
            }
        } else {
            let count = ignore_rules.len();
            tiling_obj.insert("ignore".to_string(), serde_json::json!(ignore_rules));
            count
        };

        // Drop the mutable borrow before serializing
        let _ = tiling_obj;

        // Write to a predictable temp file location
        let temp_dir = std::path::PathBuf::from("/tmp/stache-integration-tests");
        fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

        let temp_file =
            NamedTempFile::new_in(&temp_dir).expect("Failed to create temp config file");

        let config_json = serde_json::to_string_pretty(&config).unwrap();
        fs::write(temp_file.path(), &config_json).expect("Failed to write temp config file");

        eprintln!(
            "Prepared test config: {:?} -> {:?} ({} ignore rules)",
            fixture_path,
            temp_file.path(),
            total_rules
        );

        temp_file
    }

    /// Kills any running stache processes.
    pub fn kill_existing() {
        // Kill by process name
        let _ = Command::new("pkill").args(["-9", "-x", "stache"]).output();

        // Also try killall as backup
        let _ = Command::new("killall").args(["-9", "stache"]).output();

        // Give the OS time to clean up
        thread::sleep(Duration::from_millis(500));
    }

    /// Starts a new stache process with the given fixture config.
    ///
    /// This method:
    /// 1. Kills any existing stache processes
    /// 2. Loads the fixture and merges in ignore rules
    /// 3. Starts stache with the prepared config
    /// 4. Waits for the tiling manager to be initialized
    ///
    /// # Panics
    ///
    /// Panics if stache fails to start or doesn't become ready within the timeout.
    pub fn start(fixture_name: &str) -> Self {
        Self::start_with_timeout(fixture_name, DEFAULT_READY_TIMEOUT)
    }

    /// Starts stache with a custom ready timeout.
    pub fn start_with_timeout(fixture_name: &str, timeout: Duration) -> Self {
        // First, kill any existing stache processes
        Self::kill_existing();

        let binary_path = Self::binary_path();
        let config_file = Self::prepare_config(fixture_name);
        let ready = Arc::new(AtomicBool::new(false));

        eprintln!("Starting stache with config: {:?}", config_file.path());
        eprintln!("Binary path: {:?}", binary_path);

        // Start the stache process
        let mut child = Command::new(&binary_path)
            .args(["--config", config_file.path().to_str().unwrap()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start stache process");

        // Spawn a thread to monitor stderr for the ready message
        let ready_clone = Arc::clone(&ready);
        let stderr = child.stderr.take().expect("Failed to capture stderr");

        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        eprintln!("[stache] {}", line);
                        if line.contains(READY_MESSAGE) {
                            ready_clone.store(true, Ordering::SeqCst);
                        }
                    }
                    Err(e) => {
                        eprintln!("[stache stderr error] {}", e);
                        break;
                    }
                }
            }
        });

        // Also monitor stdout
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        eprintln!("[stache stdout] {}", line);
                    }
                    Err(_) => break,
                }
            }
        });

        let process = Self {
            child: Some(child),
            ready,
            _config_file: config_file,
            fixture_name: fixture_name.to_string(),
        };

        // Wait for stache to be ready
        process.wait_for_ready(timeout);

        process
    }

    /// Waits for stache to be ready.
    fn wait_for_ready(&self, timeout: Duration) {
        let start = Instant::now();

        while start.elapsed() < timeout {
            if self.ready.load(Ordering::SeqCst) {
                eprintln!("Stache is ready (took {:?})", start.elapsed());
                // Give it a bit more time to fully settle
                thread::sleep(Duration::from_millis(500));
                return;
            }

            thread::sleep(Duration::from_millis(100));
        }

        panic!(
            "Stache did not become ready within {:?}. Fixture: {}",
            timeout, self.fixture_name
        );
    }

    /// Checks if stache is ready.
    pub fn is_ready(&self) -> bool { self.ready.load(Ordering::SeqCst) }

    /// Returns the fixture name used.
    pub fn fixture_name(&self) -> &str { &self.fixture_name }

    /// Kills the stache process.
    pub fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        // Also kill by name to be sure
        Self::kill_existing();
    }
}

impl Drop for StacheProcess {
    fn drop(&mut self) {
        self.kill();
        // Note: _config_file (NamedTempFile) is automatically deleted when dropped
        // after this Drop impl completes, as Rust drops fields after the explicit drop.
    }
}
