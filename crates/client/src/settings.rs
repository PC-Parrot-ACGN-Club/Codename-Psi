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
            .init_resource::<LastBindingRejection>()
            .add_message::<SaveSettingsRequest>()
            .add_systems(
                Update,
                (
                    save_settings_on_request,
                    // Applying a locale before the catalogs have resolved would
                    // report every language as missing. The barrier alone is
                    // not enough: the resolver marks itself resolved but
                    // inserts the catalogs through a command, so this also
                    // orders after it to pick up the flush. Change detection is
                    // per-system, so the settings loaded during bootstrap still
                    // count as changed the first time this runs.
                    apply_settings
                        .after(crate::bootstrap::poll_localization)
                        .run_if(|status: Res<crate::bootstrap::BootstrapStatus>| status.is_ready()),
                ),
            );
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

    /// Binds one physical input, replacing whatever the action held on that
    /// device.
    ///
    /// Replacing rather than appending is what keeps the one-input-per-category
    /// invariant that [`Self::input_for`] and the whole settings page read
    /// through. An appended second keyboard binding would be invisible on
    /// screen -- the page shows the first -- and unremovable, while still
    /// producing the action.
    pub fn bind(&mut self, action: GameAction, input: PhysicalInput) {
        let inputs = self.bindings.entry(action).or_default();
        match inputs
            .iter_mut()
            .find(|held| held.category() == input.category())
        {
            Some(held) => *held = input,
            None => inputs.push(input),
        }
    }

    /// The input this action uses on one device, if it has one.
    ///
    /// An action holds at most one input per device category, so this is the
    /// single binding both the settings page and the on-screen key legend read.
    #[must_use]
    pub fn input_for(&self, action: GameAction, device: DeviceCategory) -> Option<&PhysicalInput> {
        self.bindings
            .get(&action)?
            .iter()
            .find(|input| input.category() == device)
    }

    /// The action of this player that already holds `input`, if any.
    ///
    /// Scoped to one player on purpose; the rule that decides whether a binding
    /// may be written spans both of them and lives on
    /// [`UserSettings::binding_conflict`].
    #[must_use]
    fn holder_of(&self, input: &PhysicalInput) -> Option<GameAction> {
        self.bindings
            .iter()
            .find_map(|(action, inputs)| inputs.contains(input).then_some(*action))
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
    /// The two rotation bindings double as the menu confirm and back keys, so
    /// their defaults are chosen for both roles at once: `J`/`K` sit under the
    /// resting fingers for rotation, and on a gamepad `South`/`East` keep the
    /// platform's usual confirm/back pair. See `client::input::ui_action_source`.
    ///
    /// Fixed bindings (directions, pause) are not listed here -- they are not
    /// user-configurable and live in `client::input`.
    #[must_use]
    pub fn for_player(player: usize) -> Self {
        let keyboard = match player {
            0 => ["KeyS", "KeyW", "KeyK", "KeyJ"],
            _ => ["ArrowDown", "ArrowUp", "Numpad2", "Numpad1"],
        };
        let gamepad = ["DPadDown", "DPadUp", "East", "South"];

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
    pub existing: BindingOwner,
    pub input: PhysicalInput,
}

/// What already holds a physical input that a capture asked for.
///
/// A configurable action names the player holding it, because a keyboard is one
/// piece of hardware shared by both locals: the action that refuses P2's key
/// press is often one of P1's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingOwner {
    Player {
        player: usize,
        action: GameAction,
    },
    /// A fixed binding -- a board or menu direction, or pause. Fixed bindings
    /// are not listed on the settings page, so this is the one owner a player
    /// cannot go and edit; the way out is to pick a different key.
    Fixed,
}

/// Which physical device a binding belongs to.
///
/// Bindings of different categories can never name the same physical input, so
/// a capture only ever considers -- and only ever collides with -- inputs of
/// its own category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceCategory {
    Keyboard,
    Gamepad,
}

/// One rebinding in progress.
///
/// Held as a resource while the settings page waits for a physical input. Its
/// presence also suspends normal input consumption, so the key being bound does
/// not simultaneously act as a menu or gameplay action.
#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct BindingCapture {
    pub player: usize,
    pub action: GameAction,
    pub device: DeviceCategory,
}

/// What one captured input did to the binding table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureOutcome {
    /// The input belongs to another device category; the capture stays open.
    Ignored,
    /// The binding was written.
    Bound,
    /// A fixed binding or another action already owns the input, so nothing was
    /// written. The settings page reports who holds it.
    Conflict(BindingConflict),
    /// The player backed out; the binding table was never touched.
    Cancelled,
}

impl BindingCapture {
    /// Opens a capture, or refuses one for a non-configurable action.
    #[must_use]
    pub fn open(player: usize, action: GameAction, device: DeviceCategory) -> Option<Self> {
        action.is_configurable().then_some(Self {
            player,
            action,
            device,
        })
    }

    /// Offers one physical input to the capture.
    ///
    /// A conflict leaves the table untouched and is final: an input that is
    /// already taken is simply refused. Taking it from its previous owner would
    /// leave that action unbound, and since the rotation bindings double as the
    /// menu confirm and back keys, an unbound rotation is a player who can no
    /// longer operate the settings page they are standing on.
    pub fn offer(
        &self,
        settings: &mut UserSettings,
        input: &PhysicalInput,
    ) -> Result<CaptureOutcome, SettingsError> {
        if input.category() != self.device {
            return Ok(CaptureOutcome::Ignored);
        }
        if settings.players.get(self.player).is_none() {
            return Err(SettingsError::UnknownPlayer(self.player));
        }
        if let Some(conflict) = settings.binding_conflict(self.player, self.action, input) {
            return Ok(CaptureOutcome::Conflict(conflict));
        }
        settings.players[self.player].bind(self.action, input.clone());
        Ok(CaptureOutcome::Bound)
    }

    /// Ends the capture on a back input, leaving the original binding in place.
    ///
    /// Consuming `self` is the point: a cancelled capture cannot go on to
    /// accept an input, so the page has to open a new one to try again.
    #[must_use]
    pub fn cancel(self) -> CaptureOutcome {
        CaptureOutcome::Cancelled
    }
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
    /// Draw a per-colour symbol inside every ball.
    ///
    /// Clearing is decided by colour alone, so the symbol carries no rule
    /// information and is off by default -- plain colour reads faster. It is
    /// the redundant cue for players who cannot separate the palette by hue,
    /// which is why the option exists rather than the symbol simply being gone.
    pub color_assist: bool,
}

impl UserSettings {
    /// Who already holds `input`, if binding it here would collide.
    ///
    /// The scope follows the hardware rather than the player. One keyboard
    /// serves both locals, so a key held by either of them is held; each player
    /// holds their own pad, so a pad button only collides inside one player's
    /// own map. Fixed bindings are checked first because they are the one owner
    /// the settings page never lists, and therefore the one a player can walk
    /// into without anything on screen having warned them.
    #[must_use]
    pub fn binding_conflict(
        &self,
        player: usize,
        action: GameAction,
        input: &PhysicalInput,
    ) -> Option<BindingConflict> {
        if !action.is_configurable() {
            return None;
        }
        let conflict = |existing| BindingConflict {
            requested: action,
            existing,
            input: input.clone(),
        };
        if crate::input::fixed_binding_claims(input, action) {
            return Some(conflict(BindingOwner::Fixed));
        }
        // A keyboard key is shared, a pad button is not.
        let shared = input.category() == DeviceCategory::Keyboard;
        (0..self.players.len())
            .filter(|slot| shared || *slot == player)
            .find_map(|slot| {
                let held = self.players[slot].holder_of(input)?;
                // Rebinding an action to the input it already holds is not a
                // collision with itself; it is a player confirming what is
                // there.
                (slot != player || held != action).then(|| {
                    conflict(BindingOwner::Player {
                        player: slot,
                        action: held,
                    })
                })
            })
    }

    /// Drop persisted bindings that break the binding invariants.
    ///
    /// Written for files that predate those invariants being enforced: before
    /// [`PlayerInputBindings::bind`] replaced instead of appended, every capture
    /// added a row, and neither the shared keyboard nor the fixed bindings were
    /// checked. Such a file makes the settings page lie -- it shows the first
    /// binding of a category while the rest fire silently -- so the repair
    /// happens on load rather than being left for the player to find.
    ///
    /// The first binding of each category survives, because that is the one the
    /// page has been showing and the one the player believes is theirs.
    pub fn normalize_bindings(&mut self) -> Vec<DroppedBinding> {
        let mut dropped = Vec::new();
        // Claims accumulate across players so the shared keyboard is settled in
        // slot order: whatever P1 holds, P2 cannot also hold.
        let mut keyboard_claims: Vec<PhysicalInput> = Vec::new();
        for (player, bindings) in self.players.iter_mut().enumerate() {
            let mut gamepad_claims: Vec<PhysicalInput> = Vec::new();
            for (action, inputs) in &mut bindings.bindings {
                let mut kept: Vec<PhysicalInput> = Vec::new();
                for input in inputs.drain(..) {
                    let claims = match input.category() {
                        DeviceCategory::Keyboard => &mut keyboard_claims,
                        DeviceCategory::Gamepad => &mut gamepad_claims,
                    };
                    let taken = claims.contains(&input)
                        || kept.iter().any(|held| held.category() == input.category())
                        || crate::input::fixed_binding_claims(&input, *action);
                    if taken {
                        dropped.push(DroppedBinding {
                            player,
                            action: *action,
                            input,
                        });
                    } else {
                        claims.push(input.clone());
                        kept.push(input);
                    }
                }
                *inputs = kept;
            }
        }
        dropped
    }
}

/// One binding that [`UserSettings::normalize_bindings`] removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedBinding {
    pub player: usize,
    pub action: GameAction,
    pub input: PhysicalInput,
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
            color_assist: false,
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
    #[error("no local player in slot {0}")]
    UnknownPlayer(usize),
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

/// The most recent refused rebinding, for the settings page to explain.
///
/// A refusal has no other visible effect -- the table is unchanged and the
/// capture is over -- so without this the player would see their key press do
/// nothing at all and conclude the page was broken.
#[derive(Debug, Default, Resource)]
pub struct LastBindingRejection(pub Option<BindingConflict>);

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
/// Input bindings are deliberately not pushed here. They reach the sampler
/// through `client::input::install_settings_bindings`, which already runs on
/// every settings change and sits early enough in the frame for a rebinding to
/// take effect on the next sampled tick.
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
