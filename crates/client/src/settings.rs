//! Serializable local user settings and persistence seams.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use game_core::input::GameAction;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::input::PhysicalInput;

pub const SETTINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Default)]
pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UserSettings>()
            .init_resource::<LastSaveError>()
            .add_message::<SaveSettingsRequest>()
            .add_systems(Update, (save_settings_on_request, apply_settings));
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowModeSetting {
    #[default]
    Windowed,
    BorderlessFullscreen,
    Fullscreen,
}

/// Amount of disposable visual motion. Rule timing is identical in both modes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimationIntensity {
    Reduced,
    #[default]
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerInputBindings {
    pub bindings: BTreeMap<GameAction, Vec<PhysicalInput>>,
}

impl PlayerInputBindings {
    pub fn actions_for<'a>(
        &'a self,
        input: &'a PhysicalInput,
    ) -> impl Iterator<Item = GameAction> + 'a {
        self.bindings
            .iter()
            .filter_map(move |(action, inputs)| inputs.contains(input).then_some(*action))
    }

    /// Reject fixed physical bindings before they enter the persisted surface.
    ///
    /// `GameAction` deserializes every variant, so a hand-edited document could
    /// otherwise carry `Left`/`Right` into the configurable binding map and be
    /// written back out on the next save.
    fn reject_fixed_bindings(&self) -> Result<(), SettingsError> {
        self.bindings
            .keys()
            .find(|action| !action.is_configurable())
            .map_or(Ok(()), |action| {
                Err(SettingsError::NonConfigurableBinding(*action))
            })
    }

    #[must_use]
    pub fn conflict(&self, action: GameAction, input: &PhysicalInput) -> Option<BindingConflict> {
        action.is_configurable().then(|| {
            self.bindings.iter().find_map(|(existing_action, inputs)| {
                (*existing_action != action && inputs.contains(input)).then(|| BindingConflict {
                    requested: action,
                    existing: *existing_action,
                    input: input.clone(),
                })
            })
        })?
    }
}

impl PlayerInputBindings {
    /// Built-in bindings for a local player slot.
    ///
    /// These are deliberately non-empty: with no settings file the game still
    /// has to be playable on both keyboard and gamepad. Keyboard keys differ
    /// per player so two locals never fight over one key; the gamepad column is
    /// the same for both because players are told apart by which pad they hold.
    ///
    /// Fixed bindings (directions, confirm, back, pause) are not listed here --
    /// they are not user-configurable and live in `client::input`.
    #[must_use]
    pub fn for_player(player: usize) -> Self {
        let keyboard = match player {
            0 => ["KeyS", "KeyW", "KeyK", "KeyJ"],
            _ => ["ArrowDown", "ArrowUp", "Numpad2", "Numpad1"],
        };
        let gamepad = ["DPadDown", "DPadUp", "South", "West"];

        let bindings = GameAction::CONFIGURABLE
            .into_iter()
            .zip(keyboard)
            .zip(gamepad)
            .map(|((action, key), button)| {
                (
                    action,
                    vec![PhysicalInput::keyboard(key), PhysicalInput::gamepad(button)],
                )
            })
            .collect();
        Self { bindings }
    }
}

impl Default for PlayerInputBindings {
    /// Player 0's bindings; use [`PlayerInputBindings::for_player`] for a slot.
    fn default() -> Self {
        Self::for_player(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingConflict {
    pub requested: GameAction,
    pub existing: GameAction,
    pub input: PhysicalInput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Resource)]
#[serde(default)]
pub struct UserSettings {
    pub schema_version: u32,
    pub language: String,
    pub window_mode: WindowModeSetting,
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub players: [PlayerInputBindings; 2],
    pub vibration: bool,
    pub animation_intensity: AnimationIntensity,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            language: "en".into(),
            window_mode: WindowModeSetting::Windowed,
            master_volume: 1.0,
            sfx_volume: 1.0,
            players: std::array::from_fn(PlayerInputBindings::for_player),
            vibration: true,
            animation_intensity: AnimationIntensity::Full,
        }
    }
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("settings parse failed: {0}")]
    Parse(String),
    #[error("unsupported settings schema {found} (supported: {supported})")]
    UnsupportedSchema { found: u32, supported: u32 },
    #[error("{0:?} is a fixed physical binding and cannot be persisted as a player binding")]
    NonConfigurableBinding(GameAction),
    #[error("settings I/O failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug)]
pub enum SettingsLoad {
    Loaded(UserSettings),
    Defaulted {
        settings: UserSettings,
        error: Option<SettingsError>,
    },
}

impl SettingsLoad {
    #[must_use]
    pub fn settings(&self) -> &UserSettings {
        match self {
            Self::Loaded(settings) | Self::Defaulted { settings, .. } => settings,
        }
    }
}

pub fn parse_settings(source: &str) -> Result<UserSettings, SettingsError> {
    let settings: UserSettings =
        ron::from_str(source).map_err(|error| SettingsError::Parse(error.to_string()))?;
    if settings.schema_version != SETTINGS_SCHEMA_VERSION {
        return Err(SettingsError::UnsupportedSchema {
            found: settings.schema_version,
            supported: SETTINGS_SCHEMA_VERSION,
        });
    }
    for player in &settings.players {
        player.reject_fixed_bindings()?;
    }
    Ok(settings)
}

pub fn serialize_settings(settings: &UserSettings) -> Result<String, SettingsError> {
    for player in &settings.players {
        player.reject_fixed_bindings()?;
    }
    ron::ser::to_string_pretty(settings, ron::ser::PrettyConfig::default())
        .map_err(|error| SettingsError::Parse(error.to_string()))
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    pub path: PathBuf,
}

impl SettingsStore {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn platform_default() -> Option<Self> {
        directories::ProjectDirs::from("org", "PC-Parrot-ACGN-Club", "Codename-Psi")
            .map(|dirs| Self::new(dirs.config_dir().join("settings.ron")))
    }

    pub fn load(&self) -> SettingsLoad {
        match std::fs::read_to_string(&self.path) {
            Ok(source) => match parse_settings(&source) {
                Ok(settings) => SettingsLoad::Loaded(settings),
                Err(error) => SettingsLoad::Defaulted {
                    settings: UserSettings::default(),
                    error: Some(error),
                },
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => SettingsLoad::Defaulted {
                settings: UserSettings::default(),
                error: None,
            },
            Err(source) => SettingsLoad::Defaulted {
                settings: UserSettings::default(),
                error: Some(SettingsError::Io {
                    path: self.path.clone(),
                    source,
                }),
            },
        }
    }

    pub fn save(&self, settings: &UserSettings) -> Result<(), SettingsError> {
        self.save_with(settings, |staged, official| {
            std::fs::rename(staged, official)
        })
    }

    /// Write-then-replace with an injectable replace step.
    ///
    /// The seam exists so tests can fail the replace *after* the staging file
    /// was written successfully — the only way to exercise the branch that has
    /// to leave the official file and the in-memory value untouched.
    pub fn save_with(
        &self,
        settings: &UserSettings,
        replace: impl FnOnce(&Path, &Path) -> std::io::Result<()>,
    ) -> Result<(), SettingsError> {
        let serialized = serialize_settings(settings)?;
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent).map_err(|source| SettingsError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
        let temporary = self.path.with_extension("ron.tmp");
        std::fs::write(&temporary, serialized).map_err(|source| SettingsError::Io {
            path: temporary.clone(),
            source,
        })?;
        replace(&temporary, &self.path).map_err(|source| SettingsError::Io {
            path: self.path.clone(),
            source,
        })
    }
}

/// Ask for the current `UserSettings` to be written to disk.
///
/// Saving is a request rather than a direct call so that UI code does not need
/// the settings path, and so a failed write is reported in one place.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct SaveSettingsRequest;

/// The outcome of the most recent save, for the settings UI to display.
#[derive(Debug, Default, Resource)]
pub struct LastSaveError(pub Option<String>);

/// Persist the current settings when a save is requested.
///
/// A failed write keeps the in-memory value and the existing file untouched;
/// only the reported error changes.
pub fn save_settings_on_request(
    mut requests: MessageReader<SaveSettingsRequest>,
    paths: Res<crate::bootstrap::BootstrapPaths>,
    settings: Res<UserSettings>,
    mut last_error: ResMut<LastSaveError>,
) {
    if requests.read().count() == 0 {
        return;
    }

    let store = match &paths.settings {
        Some(path) => Some(SettingsStore::new(path)),
        None => SettingsStore::platform_default(),
    };
    let Some(store) = store else {
        last_error.0 = Some("no platform config directory is available".into());
        return;
    };

    last_error.0 = match store.save(&settings) {
        Ok(()) => None,
        Err(error) => {
            warn!("failed to save settings: {error}");
            Some(error.to_string())
        }
    };
}

/// Push settings that other runtime systems own into those systems.
///
/// Runs when `UserSettings` changes, which covers both the value resolved at
/// startup and any later edit, so the settings screen only has to mutate the
/// resource.
///
/// Input bindings are deliberately not pushed here. The sampler is populated
/// once by `client::input::install_settings_bindings`, and re-pushing on every
/// change would overwrite a sampler that was installed deliberately. Applying
/// an edited binding belongs with the settings screen that can perform the
/// edit, which does not exist yet.
pub fn apply_settings(
    settings: Res<UserSettings>,
    mut localization: ResMut<crate::i18n::Localization>,
    mut windows: Query<&mut Window>,
) {
    if !settings.is_changed() {
        return;
    }

    if !localization.set_locale(settings.language.clone()) {
        warn!(
            "locale {} has no catalog; falling back to the default",
            settings.language
        );
    }

    for mut window in &mut windows {
        window.mode = match settings.window_mode {
            WindowModeSetting::Windowed => bevy::window::WindowMode::Windowed,
            WindowModeSetting::BorderlessFullscreen => {
                bevy::window::WindowMode::BorderlessFullscreen(MonitorSelection::Current)
            }
            WindowModeSetting::Fullscreen => bevy::window::WindowMode::Fullscreen(
                MonitorSelection::Current,
                VideoModeSelection::Current,
            ),
        };
    }
}
