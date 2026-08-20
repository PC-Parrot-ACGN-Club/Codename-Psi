//! In-memory page focus and action model.

use game_core::config::CharacterId;
use game_core::input::GameAction;

use crate::app_state::{AppState, AppTransitionCause, AppTransitionRequest, SettingsOrigin};
use crate::input::UIAction;
use crate::settings::DeviceCategory;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageItem {
    StartGame,
    Settings,
    Exit,
    SinglePlayer,
    LocalVersus,
    AiVersus,
    Lan,
    ConfirmCharacters,
    Back,
    Resume,
    Restart,
    ReturnToMainMenu,
    Rematch,
    Language,
    WindowMode,
    MasterVolume,
    SfxVolume,
    Vibration,
    AnimationIntensity,
    ColorAssist,
    /// Entry to the binding tree, on the settings root page.
    InputBindings,
    /// One local player, on the binding tree's first level.
    PlayerBindings {
        player: usize,
    },
    /// One device of one player, on the binding tree's second level.
    DeviceBindings {
        player: usize,
        device: DeviceCategory,
    },
    /// One configurable binding of one player on one device.
    Rebind {
        player: usize,
        action: GameAction,
        device: DeviceCategory,
    },
}

impl PageItem {
    /// Whether confirming this item edits a setting instead of navigating.
    ///
    /// The three binding-tree items are navigation, not settings: confirming
    /// one opens the level below it, exactly as `StartGame` opens a page.
    #[must_use]
    pub const fn is_setting(self) -> bool {
        matches!(
            self,
            Self::Language
                | Self::WindowMode
                | Self::MasterVolume
                | Self::SfxVolume
                | Self::Vibration
                | Self::AnimationIntensity
                | Self::ColorAssist
                | Self::Rebind { .. }
        )
    }
}

/// Which level of the settings tree is on screen.
///
/// The tree exists because bindings are per player *and* per device: a flat
/// list has to spell both out on every row and still mixes two players' two
/// devices into one column. Choosing the player, then the device, means each
/// screen asks one question and the four rows underneath need no qualifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsPage {
    /// General settings, plus the way into the binding tree.
    Root,
    /// Which player's bindings to edit.
    Players,
    /// Which of that player's devices to edit.
    Devices { player: usize },
    /// The four configurable actions on one device of one player.
    Bindings {
        player: usize,
        device: DeviceCategory,
    },
}

impl SettingsPage {
    /// The level confirming `item` opens, if it opens one.
    #[must_use]
    const fn opened_by(item: PageItem) -> Option<Self> {
        match item {
            PageItem::InputBindings => Some(Self::Players),
            PageItem::PlayerBindings { player } => Some(Self::Devices { player }),
            PageItem::DeviceBindings { player, device } => Some(Self::Bindings { player, device }),
            _ => None,
        }
    }
}

/// The items of one level of the settings tree, in the order it lists them.
///
/// Every level but the root ends in `Back`, which pops one level rather than
/// leaving the page: the way out of the tree is the way in, reversed.
fn settings_items(page: SettingsPage, settings_origin: Option<SettingsOrigin>) -> Vec<FocusItem> {
    let plain = |id| FocusItem::new(id, true, None::<String>);
    let mut items = match page {
        SettingsPage::Root => vec![
            plain(PageItem::Language),
            plain(PageItem::WindowMode),
            plain(PageItem::MasterVolume),
            plain(PageItem::SfxVolume),
            plain(PageItem::Vibration),
            plain(PageItem::AnimationIntensity),
            plain(PageItem::ColorAssist),
            plain(PageItem::InputBindings),
        ],
        // Every local slot, not only the ones a mode uses: the page is reachable
        // from the main menu, where no mode has been chosen yet.
        SettingsPage::Players => (0..crate::input::LOCAL_PLAYERS)
            .map(|player| plain(PageItem::PlayerBindings { player }))
            .collect(),
        SettingsPage::Devices { player } => [DeviceCategory::Keyboard, DeviceCategory::Gamepad]
            .into_iter()
            .map(|device| plain(PageItem::DeviceBindings { player, device }))
            .collect(),
        SettingsPage::Bindings { player, device } => GameAction::CONFIGURABLE
            .into_iter()
            .map(|action| {
                plain(PageItem::Rebind {
                    player,
                    action,
                    device,
                })
            })
            .collect(),
    };

    let mut back = FocusItem::new(PageItem::Back, true, None::<String>);
    // Only the root's `Back` leaves the page, so only it carries a transition.
    // The back target comes from where the page was opened, so unlike every
    // other page's `Back` the command cannot be a constant. It is still carried
    // by the item: confirming `Back` has to leave the page on its own, without
    // depending on the player also knowing the back *input*.
    if page == SettingsPage::Root
        && let Some(origin) = settings_origin
    {
        back = back.with_command(PageCommand::transition(
            origin.0,
            AppTransitionCause::SettingsClosed,
        ));
    }
    items.push(back);
    items
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageCommand {
    Transition(AppTransitionRequest),
    ExitApplication,
}

impl PageCommand {
    #[must_use]
    pub const fn transition(target: AppState, cause: AppTransitionCause) -> Self {
        Self::Transition(AppTransitionRequest { target, cause })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusItem {
    pub id: PageItem,
    pub enabled: bool,
    /// Localization key naming why the item is unavailable.
    pub unavailable_reason: Option<String>,
    pub focused: bool,
    command: Option<PageCommand>,
}

impl FocusItem {
    #[must_use]
    pub fn new(id: PageItem, enabled: bool, reason: Option<impl Into<String>>) -> Self {
        Self {
            id,
            enabled,
            unavailable_reason: reason.map(Into::into),
            focused: false,
            command: None,
        }
    }

    fn with_command(mut self, command: PageCommand) -> Self {
        self.command = Some(command);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusRing {
    items: Vec<FocusItem>,
    focused: usize,
    diagnostics: Vec<String>,
}

impl FocusRing {
    #[must_use]
    pub fn new(mut items: Vec<FocusItem>) -> Self {
        assert!(!items.is_empty(), "a focus ring needs at least one item");
        items[0].focused = true;
        Self {
            items,
            focused: 0,
            diagnostics: Vec::new(),
        }
    }

    pub fn move_focus(&mut self, action: UIAction) {
        let delta = match action {
            UIAction::Up | UIAction::Left => -1_isize,
            UIAction::Down | UIAction::Right => 1,
            UIAction::Confirm | UIAction::Back => return,
        };
        if self.items.len() == 1 {
            return;
        }
        self.items[self.focused].focused = false;
        self.focused =
            (self.focused as isize + delta).rem_euclid(self.items.len() as isize) as usize;
        self.items[self.focused].focused = true;
    }

    #[must_use]
    pub fn confirm(&self) -> Option<PageCommand> {
        let item = self.focused();
        item.enabled.then_some(item.command).flatten()
    }

    #[must_use]
    pub const fn focused_index(&self) -> usize {
        self.focused
    }

    #[must_use]
    pub fn focused(&self) -> &FocusItem {
        &self.items[self.focused]
    }

    #[must_use]
    pub fn items(&self) -> &[FocusItem] {
        &self.items
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    /// Set the enabled flag of every matching item, reporting whether any moved.
    ///
    /// The reason travels with the flag: an item is disabled *for* something,
    /// and the page says so in the row's own label.
    fn set_enabled(
        &mut self,
        matches: impl Fn(PageItem) -> bool,
        enabled: bool,
        reason: &str,
    ) -> bool {
        let mut changed = false;
        for item in self.items.iter_mut().filter(|item| matches(item.id)) {
            if item.enabled == enabled {
                continue;
            }
            item.enabled = enabled;
            item.unavailable_reason = (!enabled).then(|| reason.to_owned());
            changed = true;
        }
        changed
    }

    fn focus_item(&mut self, id: PageItem) -> Result<(), PageItem> {
        let Some(index) = self.items.iter().position(|item| item.id == id) else {
            return Err(id);
        };
        self.items[self.focused].focused = false;
        self.focused = index;
        self.items[index].focused = true;
        Ok(())
    }
}

fn action_item(id: PageItem, target: AppState, cause: AppTransitionCause) -> FocusItem {
    FocusItem::new(id, true, None::<String>).with_command(PageCommand::transition(target, cause))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageModel {
    state: AppState,
    ring: FocusRing,
    settings_origin: Option<SettingsOrigin>,
    /// The settings level on screen, and the levels above it with the item
    /// each was left on, so backing out lands on the row that was entered.
    settings_page: SettingsPage,
    settings_stack: Vec<(SettingsPage, PageItem)>,
    /// Whether a pad is connected, kept so that every ring this model builds
    /// applies it. Recomputing the ring without it is how a pad row once lost
    /// the line saying why it was unavailable.
    gamepad_available: bool,
}

impl PageModel {
    #[must_use]
    pub fn for_state(state: AppState, settings_origin: Option<SettingsOrigin>) -> Option<Self> {
        let items = match state {
            AppState::MainMenu => vec![
                action_item(
                    PageItem::StartGame,
                    AppState::ModeSelect,
                    AppTransitionCause::StartGame,
                ),
                action_item(
                    PageItem::Settings,
                    AppState::Settings,
                    AppTransitionCause::SettingsOpened,
                ),
                FocusItem::new(PageItem::Exit, true, None::<String>)
                    .with_command(PageCommand::ExitApplication),
            ],
            AppState::ModeSelect => vec![
                action_item(
                    PageItem::SinglePlayer,
                    AppState::CharacterSelect,
                    AppTransitionCause::ModeConfirmed,
                ),
                action_item(
                    PageItem::LocalVersus,
                    AppState::CharacterSelect,
                    AppTransitionCause::ModeConfirmed,
                ),
                action_item(
                    PageItem::AiVersus,
                    AppState::CharacterSelect,
                    AppTransitionCause::ModeConfirmed,
                ),
                FocusItem::new(PageItem::Lan, false, Some("mode_select.lan_unavailable")),
                action_item(
                    PageItem::Back,
                    AppState::MainMenu,
                    AppTransitionCause::BackRequested,
                ),
            ],
            AppState::CharacterSelect => vec![
                action_item(
                    PageItem::ConfirmCharacters,
                    AppState::Match,
                    AppTransitionCause::CharacterConfirmed,
                ),
                action_item(
                    PageItem::Back,
                    AppState::ModeSelect,
                    AppTransitionCause::BackRequested,
                ),
            ],
            AppState::Settings => settings_items(SettingsPage::Root, settings_origin),
            AppState::Paused => vec![
                action_item(
                    PageItem::Resume,
                    AppState::Match,
                    AppTransitionCause::ResumeRequested,
                ),
                action_item(
                    PageItem::Restart,
                    AppState::Match,
                    AppTransitionCause::RestartRequested,
                ),
                action_item(
                    PageItem::Settings,
                    AppState::Settings,
                    AppTransitionCause::SettingsOpened,
                ),
                action_item(
                    PageItem::ReturnToMainMenu,
                    AppState::MainMenu,
                    AppTransitionCause::MatchAbandoned,
                ),
            ],
            AppState::Result => vec![
                action_item(
                    PageItem::Rematch,
                    AppState::Match,
                    AppTransitionCause::RematchRequested,
                ),
                action_item(
                    PageItem::ReturnToMainMenu,
                    AppState::MainMenu,
                    AppTransitionCause::ReturnToMainMenu,
                ),
            ],
            AppState::Boot | AppState::Match => return None,
        };
        let mut model = Self {
            state,
            ring: FocusRing::new(items),
            settings_origin,
            settings_page: SettingsPage::Root,
            settings_stack: Vec::new(),
            gamepad_available: false,
        };
        // A fresh model has not been told about devices yet, and no pad is the
        // safe assumption: a row that is wrongly enabled opens a capture that
        // can never complete, while a row that is wrongly disabled is corrected
        // by the first availability report the page receives.
        model.apply_gamepad_availability();
        Some(model)
    }

    pub fn handle(&mut self, action: UIAction) -> Option<PageCommand> {
        match action {
            UIAction::Confirm => {
                let item = self.ring.focused();
                if item.enabled
                    && let Some(page) = SettingsPage::opened_by(item.id)
                {
                    self.open_settings_page(page);
                    return None;
                }
                // A sub-level's `Back` carries no command, so confirming it has
                // to pop here or it would do nothing at all.
                if item.id == PageItem::Back && self.pop_settings_page() {
                    return None;
                }
                self.ring.confirm()
            }
            UIAction::Back => self.back(),
            direction => {
                self.ring.move_focus(direction);
                None
            }
        }
    }

    /// Descend one level of the settings tree.
    fn open_settings_page(&mut self, page: SettingsPage) {
        self.settings_stack
            .push((self.settings_page, self.ring.focused().id));
        self.settings_page = page;
        self.rebuild_settings_ring();
    }

    /// Climb one level, restoring the row that was entered from.
    ///
    /// Reports whether there was a level to climb: the root's back leaves the
    /// page instead, and that is the caller's business.
    fn pop_settings_page(&mut self) -> bool {
        let Some((page, focused)) = self.settings_stack.pop() else {
            return false;
        };
        self.settings_page = page;
        self.rebuild_settings_ring();
        let _ = self.ring.focus_item(focused);
        true
    }

    /// Rebuild the ring for the current settings level.
    ///
    /// The single place a settings ring is constructed, so device availability
    /// is applied to every one of them rather than to whichever the last caller
    /// remembered.
    fn rebuild_settings_ring(&mut self) {
        self.ring = FocusRing::new(settings_items(self.settings_page, self.settings_origin));
        self.apply_gamepad_availability();
    }

    /// The settings level currently on screen.
    #[must_use]
    pub const fn settings_page(&self) -> SettingsPage {
        self.settings_page
    }

    pub fn handle_player(&mut self, _player: usize, action: UIAction) -> Option<PageCommand> {
        self.handle(action)
    }

    fn back(&mut self) -> Option<PageCommand> {
        // Inside the binding tree, back means one level up. Only the root level
        // has a page to return to.
        if self.pop_settings_page() {
            return None;
        }
        match self.state {
            AppState::ModeSelect => Some(PageCommand::transition(
                AppState::MainMenu,
                AppTransitionCause::BackRequested,
            )),
            AppState::CharacterSelect => Some(PageCommand::transition(
                AppState::ModeSelect,
                AppTransitionCause::BackRequested,
            )),
            AppState::Settings => self.settings_origin.map(|origin| {
                PageCommand::transition(origin.0, AppTransitionCause::SettingsClosed)
            }),
            AppState::Paused => Some(PageCommand::transition(
                AppState::Match,
                AppTransitionCause::ResumeRequested,
            )),
            _ => None,
        }
    }

    pub fn focus_item(&mut self, item: PageItem) -> Result<(), PageItem> {
        self.ring.focus_item(item)
    }

    #[must_use]
    pub fn focused(&self) -> &FocusItem {
        self.ring.focused()
    }

    #[must_use]
    pub fn items(&self) -> &[FocusItem] {
        self.ring.items()
    }

    /// The state this page belongs to.
    #[must_use]
    pub const fn state(&self) -> AppState {
        self.state
    }

    #[must_use]
    pub const fn focused_index(&self) -> usize {
        self.ring.focused_index()
    }

    /// Enable or disable the pad rows, and say whether that changed anything.
    ///
    /// A capture waits for an input from its own device category and ignores
    /// everything else, so opening a pad capture with no pad plugged in leaves a
    /// row that can never complete. Disabling the row is what keeps the player
    /// from walking into it; the tree still lists it, because the settings page
    /// is a list of what the game can be told, not a list of what is plugged in
    /// right now.
    ///
    /// One flag for both players: a capture accepts input from any connected
    /// pad, so per-slot availability would claim a precision that is not there.
    /// A player without a pad can still configure the one they are about to
    /// hold, which is the point of the rows being listed at all.
    pub fn set_gamepad_available(&mut self, available: bool) -> bool {
        if self.gamepad_available == available {
            // The ring already agrees; nothing on screen has to be redrawn.
            return false;
        }
        self.gamepad_available = available;
        self.apply_gamepad_availability()
    }

    fn apply_gamepad_availability(&mut self) -> bool {
        self.ring.set_enabled(
            |id| {
                matches!(
                    id,
                    PageItem::DeviceBindings {
                        device: DeviceCategory::Gamepad,
                        ..
                    } | PageItem::Rebind {
                        device: DeviceCategory::Gamepad,
                        ..
                    }
                )
            },
            self.gamepad_available,
            "settings.no_gamepad",
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MatchMode {
    #[default]
    SinglePlayer,
    LocalVersus,
    /// Both sides are driven by the AI; nobody plays.
    ///
    /// It exists to make the presentation observable: a match runs to its end
    /// without a hand on the keyboard, so the animations can be watched
    /// instead of played through.
    AiVersus,
}

impl MatchMode {
    /// Whether one local player picks the characters for both slots.
    ///
    /// True wherever slot 1 is not a second person at the keyboard: the AI
    /// opponent of [`Self::SinglePlayer`] and both AI sides of
    /// [`Self::AiVersus`] still need a character, and the one player present
    /// chooses it.
    #[must_use]
    pub const fn one_selector(self) -> bool {
        matches!(self, Self::SinglePlayer | Self::AiVersus)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterSelectPage {
    mode: MatchMode,
    characters: Vec<CharacterId>,
    focused: [usize; 2],
    selected: [Option<CharacterId>; 2],
}

impl CharacterSelectPage {
    #[must_use]
    pub fn new(mode: MatchMode, characters: Vec<CharacterId>) -> Self {
        assert!(!characters.is_empty(), "character selection needs a roster");
        // One selector modes hand both slots to the same player, so the
        // shortest path is Confirm, Confirm. Slot 1 starts one roster entry
        // ahead of slot 0 so that path does not land on the same character
        // twice.
        let focused = if mode.one_selector() {
            [0, 1 % characters.len()]
        } else {
            [0, 0]
        };
        Self {
            mode,
            characters,
            focused,
            selected: [None, None],
        }
    }

    pub fn handle_player(&mut self, player: usize, action: UIAction) {
        if player >= 2 || (self.mode.one_selector() && player == 1) {
            return;
        }
        let controlled_slot =
            if self.mode.one_selector() && player == 0 && self.selected[0].is_some() {
                1
            } else {
                player
            };
        match action {
            UIAction::Up | UIAction::Left => {
                self.focused[controlled_slot] =
                    (self.focused[controlled_slot] + self.characters.len() - 1)
                        % self.characters.len();
            }
            UIAction::Down | UIAction::Right => {
                self.focused[controlled_slot] =
                    (self.focused[controlled_slot] + 1) % self.characters.len();
            }
            UIAction::Confirm => {
                self.selected[controlled_slot] =
                    Some(self.characters[self.focused[controlled_slot]].clone());
            }
            UIAction::Back => {
                self.selected[controlled_slot] = None;
                if self.mode.one_selector() && controlled_slot == 1 {
                    self.selected[0] = None;
                }
            }
        }
    }

    #[must_use]
    pub fn focused_index(&self, player: usize) -> Option<usize> {
        self.focused.get(player).copied()
    }

    /// The roster this page offers, in display order.
    #[must_use]
    pub fn characters(&self) -> &[CharacterId] {
        &self.characters
    }

    #[must_use]
    pub fn confirm_enabled(&self) -> bool {
        self.selected.iter().all(Option::is_some)
    }

    #[must_use]
    pub fn confirm_selection(&self) -> Option<PageCommand> {
        self.confirm_enabled().then(|| {
            PageCommand::transition(AppState::Match, AppTransitionCause::CharacterConfirmed)
        })
    }

    #[must_use]
    pub fn selected(&self) -> [Option<&CharacterId>; 2] {
        std::array::from_fn(|slot| self.selected[slot].as_ref())
    }
}
