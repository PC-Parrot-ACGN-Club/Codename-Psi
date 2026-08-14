//! In-memory page focus and action model.

use game_core::config::CharacterId;

use crate::app_state::{AppState, AppTransitionCause, AppTransitionRequest, SettingsOrigin};
use crate::input::UIAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageItem {
    StartGame,
    Settings,
    Exit,
    SinglePlayer,
    LocalVersus,
    Lan,
    ConfirmCharacters,
    Back,
    Resume,
    Restart,
    ReturnToMainMenu,
    Rematch,
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
                FocusItem::new(PageItem::Lan, false, Some("available in R2")),
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
            AppState::Settings => vec![FocusItem::new(PageItem::Back, true, None::<String>)],
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
        Some(Self {
            state,
            ring: FocusRing::new(items),
            settings_origin,
        })
    }

    pub fn handle(&mut self, action: UIAction) -> Option<PageCommand> {
        match action {
            UIAction::Confirm => self.ring.confirm(),
            UIAction::Back => self.back(),
            direction => {
                self.ring.move_focus(direction);
                None
            }
        }
    }

    pub fn handle_player(&mut self, _player: usize, action: UIAction) -> Option<PageCommand> {
        self.handle(action)
    }

    fn back(&self) -> Option<PageCommand> {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    SinglePlayer,
    LocalVersus,
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
        Self {
            mode,
            characters,
            focused: [0; 2],
            selected: [None, None],
        }
    }

    pub fn handle_player(&mut self, player: usize, action: UIAction) {
        if player >= 2 || (self.mode == MatchMode::SinglePlayer && player == 1) {
            return;
        }
        let controlled_slot =
            if self.mode == MatchMode::SinglePlayer && player == 0 && self.selected[0].is_some() {
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
                if self.mode == MatchMode::SinglePlayer && controlled_slot == 1 {
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
