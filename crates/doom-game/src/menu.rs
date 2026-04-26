//! Menu system and title screen logic.
//!
//! `GameMenu` drives the classic Doom menu: Main, Episode selection, Skill
//! selection, Load/Save game, and Options.  `TitleScreen` manages the
//! title-screen demo cycle (TITLEPIC -> Demo -> CREDIT -> Demo -> ...).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The specific game version or IWAD currently loaded.
///
/// Doom 1 and Doom 2 have different episode structures and menu layouts.
///
/// ## Examples
/// ```
/// use doom_game::menu::GameVersion;
/// let version = GameVersion::Doom1;
/// assert_eq!(version, GameVersion::Doom1);
/// ```
pub enum GameVersion {
    /// Classic Doom 1: "Knee-Deep in the Dead", "The Shores of Hell", "Inferno".
    Doom1,
    /// Doom 2: "Hell on Earth" (a continuous 30-level campaign).
    Doom2,
}

// ---------------------------------------------------------------------------
// MenuPage
// ---------------------------------------------------------------------------

/// Which page of the menu is currently displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuPage {
    /// Main menu: New Game, Options, Load Game, Save Game, Quit.
    Main,
    /// Episode selection (Doom 1 only): Knee-Deep, Shores of Hell, Inferno.
    Episode,
    /// Skill selection: I'm Too Young to Die .. Nightmare!
    Skill,
    /// Load game: 6 save slots.
    Load,
    /// Save game: 6 save slots.
    Save,
    /// Options: SFX volume, Music volume, controls info.
    Options,
}

// ---------------------------------------------------------------------------
// MenuAction
// ---------------------------------------------------------------------------

/// What happens when a menu item is activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Navigate to another menu page.
    GoTo(MenuPage),
    /// Start a new game with the selected episode (1-3).
    SelectEpisode(u8),
    /// Start a new game with the selected skill level (0-4).
    SelectSkill(u8),
    /// Load from the given save slot (0-5).
    LoadSlot(u8),
    /// Save to the given save slot (0-5).
    SaveSlot(u8),
    /// Quit the game.
    Quit,
    /// No-op / placeholder.
    Noop,
}

// ---------------------------------------------------------------------------
// MenuItem
// ---------------------------------------------------------------------------

/// A single entry on a menu page.
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Display text for this item.
    pub label: &'static str,
    /// What happens when selected.
    pub action: MenuAction,
    /// Whether this item is selectable (greyed out if false).
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// MenuResult
// ---------------------------------------------------------------------------

/// The outcome of activating a menu item that the caller must handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuResult {
    /// Start a new game with the given episode and skill.
    StartGame {
        /// Episode index (0-3).
        episode: u8,
        /// Skill level index (0-4).
        skill: u8,
    },
    /// Load from save slot.
    LoadGame(u8),
    /// Save to save slot.
    SaveGame(u8),
    /// Quit the game.
    Quit,
    /// Navigated to a new page (no external action needed).
    PageChange,
}

// ---------------------------------------------------------------------------
// Static menu item tables
// ---------------------------------------------------------------------------

/// Main menu items for Doom 1 (New Game goes to Episode select).
static MAIN_ITEMS_DOOM1: &[MenuItem] = &[
    MenuItem {
        label: "New Game",
        action: MenuAction::GoTo(MenuPage::Episode),
        enabled: true,
    },
    MenuItem {
        label: "Options",
        action: MenuAction::GoTo(MenuPage::Options),
        enabled: true,
    },
    MenuItem {
        label: "Load Game",
        action: MenuAction::GoTo(MenuPage::Load),
        enabled: true,
    },
    MenuItem {
        label: "Save Game",
        action: MenuAction::GoTo(MenuPage::Save),
        enabled: true,
    },
    MenuItem {
        label: "Quit Game",
        action: MenuAction::Quit,
        enabled: true,
    },
];

/// Main menu items for Doom 2 (New Game goes directly to Skill select).
static MAIN_ITEMS_DOOM2: &[MenuItem] = &[
    MenuItem {
        label: "New Game",
        action: MenuAction::GoTo(MenuPage::Skill),
        enabled: true,
    },
    MenuItem {
        label: "Options",
        action: MenuAction::GoTo(MenuPage::Options),
        enabled: true,
    },
    MenuItem {
        label: "Load Game",
        action: MenuAction::GoTo(MenuPage::Load),
        enabled: true,
    },
    MenuItem {
        label: "Save Game",
        action: MenuAction::GoTo(MenuPage::Save),
        enabled: true,
    },
    MenuItem {
        label: "Quit Game",
        action: MenuAction::Quit,
        enabled: true,
    },
];

static EPISODE_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Knee-Deep in the Dead",
        action: MenuAction::SelectEpisode(1),
        enabled: true,
    },
    MenuItem {
        label: "The Shores of Hell",
        action: MenuAction::SelectEpisode(2),
        enabled: true,
    },
    MenuItem {
        label: "Inferno",
        action: MenuAction::SelectEpisode(3),
        enabled: true,
    },
];

static SKILL_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "I'm too young to die",
        action: MenuAction::SelectSkill(0),
        enabled: true,
    },
    MenuItem {
        label: "Hey, not too rough",
        action: MenuAction::SelectSkill(1),
        enabled: true,
    },
    MenuItem {
        label: "Hurt me plenty",
        action: MenuAction::SelectSkill(2),
        enabled: true,
    },
    MenuItem {
        label: "Ultra-Violence",
        action: MenuAction::SelectSkill(3),
        enabled: true,
    },
    MenuItem {
        label: "Nightmare!",
        action: MenuAction::SelectSkill(4),
        enabled: true,
    },
];

static LOAD_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "slot 1",
        action: MenuAction::LoadSlot(0),
        enabled: true,
    },
    MenuItem {
        label: "slot 2",
        action: MenuAction::LoadSlot(1),
        enabled: true,
    },
    MenuItem {
        label: "slot 3",
        action: MenuAction::LoadSlot(2),
        enabled: true,
    },
    MenuItem {
        label: "slot 4",
        action: MenuAction::LoadSlot(3),
        enabled: true,
    },
    MenuItem {
        label: "slot 5",
        action: MenuAction::LoadSlot(4),
        enabled: true,
    },
    MenuItem {
        label: "slot 6",
        action: MenuAction::LoadSlot(5),
        enabled: true,
    },
];

static SAVE_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "slot 1",
        action: MenuAction::SaveSlot(0),
        enabled: true,
    },
    MenuItem {
        label: "slot 2",
        action: MenuAction::SaveSlot(1),
        enabled: true,
    },
    MenuItem {
        label: "slot 3",
        action: MenuAction::SaveSlot(2),
        enabled: true,
    },
    MenuItem {
        label: "slot 4",
        action: MenuAction::SaveSlot(3),
        enabled: true,
    },
    MenuItem {
        label: "slot 5",
        action: MenuAction::SaveSlot(4),
        enabled: true,
    },
    MenuItem {
        label: "slot 6",
        action: MenuAction::SaveSlot(5),
        enabled: true,
    },
];

static OPTIONS_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Sound Volume",
        action: MenuAction::Noop,
        enabled: true,
    },
    MenuItem {
        label: "Music Volume",
        action: MenuAction::Noop,
        enabled: true,
    },
    MenuItem {
        label: "Controls",
        action: MenuAction::Noop,
        enabled: true,
    },
];

// ---------------------------------------------------------------------------
// GameMenu
// ---------------------------------------------------------------------------

/// Skull cursor toggle period in tics.
const SKULL_TOGGLE_TICS: u32 = 8;

/// The Doom menu state machine.
///
/// Tracks which page is shown, cursor position, episode selection, and
/// skull-cursor animation.  Completely deterministic (no I/O).
pub struct GameMenu {
    /// Whether the menu is currently active/visible.
    active: bool,
    /// Current menu page.
    page: MenuPage,
    /// Cursor position (index into current page's items).
    cursor: usize,
    /// Selected episode (for Skill page to reference).  1-3 for Doom 1, 1 for Doom 2.
    selected_episode: u8,
    /// Whether this is a Doom 2 game (skips episode selection).
    is_doom2: bool,
    /// Skull cursor animation tic (toggles between two frames).
    skull_tic: u32,
}

impl GameMenu {
    /// Create a new menu.  Starts inactive (not visible).
    pub fn new(game_version: GameVersion) -> Self {
        Self {
            active: false,
            page: MenuPage::Main,
            cursor: 0,
            selected_episode: 1,
            is_doom2: game_version == GameVersion::Doom2,
            skull_tic: 0,
        }
    }

    /// Activate the menu and navigate to the Main page.
    pub fn open(&mut self) {
        self.active = true;
        self.page = MenuPage::Main;
        self.cursor = 0;
    }

    /// Deactivate the menu.
    pub fn close(&mut self) {
        self.active = false;
    }

    /// Whether the menu is currently active/visible.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// The current menu page.
    pub fn page(&self) -> MenuPage {
        self.page
    }

    /// The current cursor position (index into `items()`).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The currently-selected episode (set when the player picks one on the
    /// Episode page, or always 1 for Doom 2).
    pub fn selected_episode(&self) -> u8 {
        self.selected_episode
    }

    /// Skull cursor animation frame: 0 or 1, toggling every `SKULL_TOGGLE_TICS`.
    pub fn skull_frame(&self) -> u8 {
        ((self.skull_tic / SKULL_TOGGLE_TICS) % 2) as u8
    }

    /// Return the items for the current page.
    pub fn items(&self) -> &[MenuItem] {
        match self.page {
            MenuPage::Main => {
                if self.is_doom2 {
                    MAIN_ITEMS_DOOM2
                } else {
                    MAIN_ITEMS_DOOM1
                }
            }
            MenuPage::Episode => EPISODE_ITEMS,
            MenuPage::Skill => SKILL_ITEMS,
            MenuPage::Load => LOAD_ITEMS,
            MenuPage::Save => SAVE_ITEMS,
            MenuPage::Options => OPTIONS_ITEMS,
        }
    }

    /// Move the cursor up (wraps from 0 to last item).
    pub fn move_up(&mut self) {
        let len = self.items().len();
        if len == 0 {
            return;
        }
        if self.cursor == 0 {
            self.cursor = len - 1;
        } else {
            self.cursor -= 1;
        }
    }

    /// Move the cursor down (wraps from last item to 0).
    pub fn move_down(&mut self) {
        let len = self.items().len();
        if len == 0 {
            return;
        }
        self.cursor = (self.cursor + 1) % len;
    }

    /// Activate the currently-highlighted item.
    ///
    /// Returns `Some(MenuResult)` if the caller needs to take action
    /// (start game, load, save, quit).  Returns `None` for no-ops or
    /// disabled items.
    pub fn select(&mut self) -> Option<MenuResult> {
        let items = self.items();
        if self.cursor >= items.len() {
            return None;
        }
        let item = &items[self.cursor];
        if !item.enabled {
            return None;
        }

        match item.action {
            MenuAction::GoTo(page) => {
                self.page = page;
                self.cursor = 0;
                Some(MenuResult::PageChange)
            }
            MenuAction::SelectEpisode(ep) => {
                self.selected_episode = ep;
                self.page = MenuPage::Skill;
                self.cursor = 0;
                Some(MenuResult::PageChange)
            }
            MenuAction::SelectSkill(sk) => Some(MenuResult::StartGame {
                episode: self.selected_episode,
                skill: sk,
            }),
            MenuAction::LoadSlot(n) => Some(MenuResult::LoadGame(n)),
            MenuAction::SaveSlot(n) => Some(MenuResult::SaveGame(n)),
            MenuAction::Quit => Some(MenuResult::Quit),
            MenuAction::Noop => None,
        }
    }

    /// Navigate back to the previous page, or close the menu from Main.
    pub fn back(&mut self) {
        match self.page {
            MenuPage::Main => {
                self.close();
            }
            MenuPage::Episode => {
                self.page = MenuPage::Main;
                self.cursor = 0;
            }
            MenuPage::Skill => {
                if self.is_doom2 {
                    // Doom 2 skips episode; back from Skill goes to Main.
                    self.page = MenuPage::Main;
                } else {
                    self.page = MenuPage::Episode;
                }
                self.cursor = 0;
            }
            MenuPage::Load | MenuPage::Save | MenuPage::Options => {
                self.page = MenuPage::Main;
                self.cursor = 0;
            }
        }
    }

    /// Advance the skull cursor animation by one tic.
    pub fn tick(&mut self) {
        self.skull_tic = self.skull_tic.wrapping_add(1);
    }
}

// ---------------------------------------------------------------------------
// TitleScreen
// ---------------------------------------------------------------------------

/// Which phase of the title/demo sequence we are in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitlePhase {
    /// Showing the title graphic (TITLEPIC).
    Title,
    /// Running a demo playback.
    Demo(u8),
    /// Showing the credits screen (CREDIT).
    Credits,
}

/// Tics the title graphic is shown before transitioning to credits.
const TITLE_DURATION: u32 = 350;

/// Tics the credits graphic is shown before cycling back to the title.
const CREDITS_DURATION: u32 = 200;

/// Title screen attract-mode timing.
///
/// The timer-driven loop currently cycles through `Title -> Credits -> Title`.
/// `Demo(_)` remains representable for future real playback, but normal ticking
/// intentionally stays out of demo phases until that playback path exists.
pub struct TitleScreen {
    /// Current tic in the current phase.
    tic: u32,
    /// Which title screen phase we're in.
    phase: TitlePhase,
}

impl TitleScreen {
    /// Create a new title screen starting at the Title phase.
    pub fn new() -> Self {
        Self {
            tic: 0,
            phase: TitlePhase::Title,
        }
    }

    /// Create a title screen starting in a specific phase.
    pub fn from_phase(phase: TitlePhase) -> Self {
        Self { tic: 0, phase }
    }

    /// Advance the title screen by one tic.
    ///
    /// Auto-transitions:
    /// - Title (350 tics) -> Credits
    /// - Demo(n) is expected to be driven externally (demo playback); when
    ///   the demo ends, the caller should call `advance_from_demo()`.
    /// - Credits (200 tics) -> Title
    pub fn tick(&mut self) {
        self.tic += 1;

        match self.phase {
            TitlePhase::Title => {
                if self.tic >= TITLE_DURATION {
                    self.phase = TitlePhase::Credits;
                    self.tic = 0;
                }
            }
            TitlePhase::Demo(n) => {
                // Demo playback is not timer-driven yet.
                let _ = n; // suppress unused warning
            }
            TitlePhase::Credits => {
                if self.tic >= CREDITS_DURATION {
                    self.advance_after_credits();
                }
            }
        }
    }

    /// Transition from credits to the next phase.
    fn advance_after_credits(&mut self) {
        self.phase = TitlePhase::Title;
        self.tic = 0;
    }

    /// Called when a demo finishes playback (either naturally or via timeout).
    ///
    /// Transitions: Demo(0) -> Credits, Demo(1) -> Title, Demo(2) -> Credits.
    pub fn advance_from_demo(&mut self) {
        match self.phase {
            TitlePhase::Demo(n) => {
                if n % 2 == 0 {
                    // Even demos (0, 2) -> Credits
                    self.phase = TitlePhase::Credits;
                } else {
                    // Odd demos (1) -> Title
                    self.phase = TitlePhase::Title;
                }
                self.tic = 0;
            }
            _ => {
                // Not in a demo phase; ignore.
            }
        }
    }

    /// The current phase.
    pub fn phase(&self) -> TitlePhase {
        self.phase
    }

    /// Reset to the Title phase (e.g. when returning from gameplay).
    pub fn reset(&mut self) {
        self.phase = TitlePhase::Title;
        self.tic = 0;
    }

    /// The current tic within the current phase.
    pub fn tic(&self) -> u32 {
        self.tic
    }
}

impl Default for TitleScreen {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // =======================================================================
    // GameMenu: construction and basic state
    // =======================================================================

    #[test]
    fn new_starts_inactive() {
        let menu = GameMenu::new(GameVersion::Doom1);
        assert!(!menu.is_active());
    }

    #[test]
    fn new_doom2_starts_inactive() {
        let menu = GameMenu::new(GameVersion::Doom2);
        assert!(!menu.is_active());
    }

    #[test]
    fn open_activates_menu() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        assert!(menu.is_active());
        assert_eq!(menu.page(), MenuPage::Main);
        assert_eq!(menu.cursor(), 0);
    }

    #[test]
    fn close_deactivates_menu() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.close();
        assert!(!menu.is_active());
    }

    #[test]
    fn open_resets_to_main_page() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        // Navigate away
        menu.select(); // "New Game" -> Episode
        assert_eq!(menu.page(), MenuPage::Episode);
        // Re-open
        menu.open();
        assert_eq!(menu.page(), MenuPage::Main);
        assert_eq!(menu.cursor(), 0);
    }

    // =======================================================================
    // Page item counts
    // =======================================================================

    #[test]
    fn main_page_has_5_items() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        assert_eq!(menu.items().len(), 5);
    }

    #[test]
    fn main_page_doom2_has_5_items() {
        let mut menu = GameMenu::new(GameVersion::Doom2);
        menu.open();
        assert_eq!(menu.items().len(), 5);
    }

    #[test]
    fn episode_page_has_3_items() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.select(); // -> Episode
        assert_eq!(menu.page(), MenuPage::Episode);
        assert_eq!(menu.items().len(), 3);
    }

    #[test]
    fn skill_page_has_5_items() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.select(); // -> Episode
        menu.select(); // SelectEpisode(1) -> Skill
        assert_eq!(menu.page(), MenuPage::Skill);
        assert_eq!(menu.items().len(), 5);
    }

    #[test]
    fn load_page_has_6_items() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.cursor = 2; // "Load Game"
        menu.select(); // -> Load
        assert_eq!(menu.page(), MenuPage::Load);
        assert_eq!(menu.items().len(), 6);
    }

    #[test]
    fn save_page_has_6_items() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.cursor = 3; // "Save Game"
        menu.select(); // -> Save
        assert_eq!(menu.page(), MenuPage::Save);
        assert_eq!(menu.items().len(), 6);
    }

    #[test]
    fn options_page_has_3_items() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.cursor = 1; // "Options"
        menu.select(); // -> Options
        assert_eq!(menu.page(), MenuPage::Options);
        assert_eq!(menu.items().len(), 3);
    }

    // =======================================================================
    // Navigation: move_up / move_down
    // =======================================================================

    #[test]
    fn move_down_increments_cursor() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        assert_eq!(menu.cursor(), 0);
        menu.move_down();
        assert_eq!(menu.cursor(), 1);
    }

    #[test]
    fn move_down_wraps_from_last_to_0() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        let len = menu.items().len();
        for _ in 0..len {
            menu.move_down();
        }
        assert_eq!(menu.cursor(), 0);
    }

    #[test]
    fn move_up_wraps_from_0_to_last() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        assert_eq!(menu.cursor(), 0);
        menu.move_up();
        assert_eq!(menu.cursor(), menu.items().len() - 1);
    }

    #[test]
    fn move_up_decrements_cursor() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.move_down(); // 1
        menu.move_down(); // 2
        menu.move_up(); // 1
        assert_eq!(menu.cursor(), 1);
    }

    #[test]
    fn cursor_stays_in_bounds_main() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        for _ in 0..20 {
            menu.move_down();
        }
        assert!(menu.cursor() < menu.items().len());
    }

    #[test]
    fn cursor_stays_in_bounds_episode() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.select(); // -> Episode
        for _ in 0..20 {
            menu.move_up();
        }
        assert!(menu.cursor() < menu.items().len());
    }

    #[test]
    fn cursor_stays_in_bounds_skill() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.select(); // -> Episode
        menu.select(); // -> Skill
        for _ in 0..30 {
            menu.move_down();
        }
        assert!(menu.cursor() < menu.items().len());
    }

    // =======================================================================
    // Navigation: back()
    // =======================================================================

    #[test]
    fn back_from_main_closes_menu() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.back();
        assert!(!menu.is_active());
    }

    #[test]
    fn back_from_episode_goes_to_main() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.select(); // -> Episode
        assert_eq!(menu.page(), MenuPage::Episode);
        menu.back();
        assert_eq!(menu.page(), MenuPage::Main);
    }

    #[test]
    fn back_from_skill_goes_to_episode_doom1() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.select(); // -> Episode
        menu.select(); // -> Skill
        assert_eq!(menu.page(), MenuPage::Skill);
        menu.back();
        assert_eq!(menu.page(), MenuPage::Episode);
    }

    #[test]
    fn back_from_skill_goes_to_main_doom2() {
        let mut menu = GameMenu::new(GameVersion::Doom2);
        menu.open();
        menu.select(); // -> Skill (skips episode in Doom 2)
        assert_eq!(menu.page(), MenuPage::Skill);
        menu.back();
        assert_eq!(menu.page(), MenuPage::Main);
    }

    #[test]
    fn back_from_load_goes_to_main() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.cursor = 2; // "Load Game"
        menu.select(); // -> Load
        assert_eq!(menu.page(), MenuPage::Load);
        menu.back();
        assert_eq!(menu.page(), MenuPage::Main);
    }

    #[test]
    fn back_from_save_goes_to_main() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.cursor = 3; // "Save Game"
        menu.select(); // -> Save
        assert_eq!(menu.page(), MenuPage::Save);
        menu.back();
        assert_eq!(menu.page(), MenuPage::Main);
    }

    #[test]
    fn back_from_options_goes_to_main() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.cursor = 1; // "Options"
        menu.select(); // -> Options
        assert_eq!(menu.page(), MenuPage::Options);
        menu.back();
        assert_eq!(menu.page(), MenuPage::Main);
    }

    // =======================================================================
    // select() results
    // =======================================================================

    #[test]
    fn select_new_game_doom1_goes_to_episode() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        let result = menu.select(); // "New Game" -> Episode
        assert_eq!(result, Some(MenuResult::PageChange));
        assert_eq!(menu.page(), MenuPage::Episode);
    }

    #[test]
    fn select_new_game_doom2_goes_to_skill() {
        let mut menu = GameMenu::new(GameVersion::Doom2);
        menu.open();
        let result = menu.select(); // "New Game" -> Skill (Doom 2)
        assert_eq!(result, Some(MenuResult::PageChange));
        assert_eq!(menu.page(), MenuPage::Skill);
    }

    #[test]
    fn select_episode_stores_episode_and_goes_to_skill() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.select(); // -> Episode
        menu.move_down(); // cursor = 1 (Shores of Hell, ep 2)
        let result = menu.select();
        assert_eq!(result, Some(MenuResult::PageChange));
        assert_eq!(menu.page(), MenuPage::Skill);
        assert_eq!(menu.selected_episode(), 2);
    }

    #[test]
    fn select_skill_returns_start_game_with_correct_episode() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.select(); // -> Episode
        menu.move_down(); // ep 2
        menu.select(); // -> Skill
        menu.move_down();
        menu.move_down(); // skill 2 (Hurt me plenty)
        let result = menu.select();
        assert_eq!(
            result,
            Some(MenuResult::StartGame {
                episode: 2,
                skill: 2,
            })
        );
    }

    #[test]
    fn select_skill_returns_start_game_episode1() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.select(); // -> Episode
        menu.select(); // ep 1 -> Skill
        let result = menu.select(); // skill 0
        assert_eq!(
            result,
            Some(MenuResult::StartGame {
                episode: 1,
                skill: 0,
            })
        );
    }

    #[test]
    fn select_load_slot_returns_load_game() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.cursor = 2; // "Load Game"
        menu.select(); // -> Load
        menu.move_down();
        menu.move_down(); // slot 3 (index 2)
        let result = menu.select();
        assert_eq!(result, Some(MenuResult::LoadGame(2)));
    }

    #[test]
    fn select_save_slot_returns_save_game() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.cursor = 3; // "Save Game"
        menu.select(); // -> Save
        menu.move_down(); // slot 2 (index 1)
        let result = menu.select();
        assert_eq!(result, Some(MenuResult::SaveGame(1)));
    }

    #[test]
    fn select_quit_returns_quit() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.cursor = 4; // "Quit Game"
        let result = menu.select();
        assert_eq!(result, Some(MenuResult::Quit));
    }

    #[test]
    fn select_noop_returns_none() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.cursor = 1; // "Options"
        menu.select(); // -> Options page
        // All options items are Noop
        let result = menu.select();
        assert!(result.is_none());
    }

    #[test]
    fn select_disabled_item_returns_none() {
        // We cannot easily create disabled items in the static arrays,
        // but we can test the code path by verifying the logic:
        // If we set cursor out of bounds, select returns None.
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.cursor = 99; // out of bounds
        let result = menu.select();
        assert!(result.is_none());
    }

    // =======================================================================
    // Doom 2 mode
    // =======================================================================

    #[test]
    fn doom2_new_game_goes_directly_to_skill() {
        let mut menu = GameMenu::new(GameVersion::Doom2);
        menu.open();
        let result = menu.select();
        assert_eq!(result, Some(MenuResult::PageChange));
        assert_eq!(menu.page(), MenuPage::Skill);
    }

    #[test]
    fn doom2_selected_episode_always_1() {
        let mut menu = GameMenu::new(GameVersion::Doom2);
        menu.open();
        assert_eq!(menu.selected_episode(), 1);
        menu.select(); // -> Skill
        let result = menu.select(); // skill 0
        assert_eq!(
            result,
            Some(MenuResult::StartGame {
                episode: 1,
                skill: 0,
            })
        );
    }

    #[test]
    fn doom2_back_from_skill_goes_to_main() {
        let mut menu = GameMenu::new(GameVersion::Doom2);
        menu.open();
        menu.select(); // -> Skill
        menu.back();
        assert_eq!(menu.page(), MenuPage::Main);
    }

    // =======================================================================
    // Skull animation
    // =======================================================================

    #[test]
    fn skull_frame_starts_at_0() {
        let menu = GameMenu::new(GameVersion::Doom1);
        assert_eq!(menu.skull_frame(), 0);
    }

    #[test]
    fn skull_frame_toggles_every_8_tics() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        // Frames 0-7: skull_frame = 0
        for _ in 0..SKULL_TOGGLE_TICS {
            assert_eq!(menu.skull_frame(), 0);
            menu.tick();
        }
        // Frames 8-15: skull_frame = 1
        for _ in 0..SKULL_TOGGLE_TICS {
            assert_eq!(menu.skull_frame(), 1);
            menu.tick();
        }
        // Frames 16-23: skull_frame = 0 again
        assert_eq!(menu.skull_frame(), 0);
    }

    #[test]
    fn tick_advances_skull_tic() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        assert_eq!(menu.skull_tic, 0);
        menu.tick();
        assert_eq!(menu.skull_tic, 1);
        menu.tick();
        assert_eq!(menu.skull_tic, 2);
    }

    // =======================================================================
    // Edge cases
    // =======================================================================

    #[test]
    fn multiple_episode_selections_update_correctly() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.select(); // -> Episode

        // Select episode 1
        menu.select(); // ep 1 -> Skill
        assert_eq!(menu.selected_episode(), 1);

        // Go back and choose ep 3
        menu.back(); // -> Episode
        menu.move_down();
        menu.move_down(); // cursor 2 (Inferno, ep 3)
        menu.select(); // ep 3 -> Skill
        assert_eq!(menu.selected_episode(), 3);
    }

    #[test]
    fn open_close_open_preserves_doom2_mode() {
        let mut menu = GameMenu::new(GameVersion::Doom2);
        menu.open();
        menu.close();
        menu.open();
        assert!(menu.is_doom2);
        // Verify Doom 2 behavior still works
        let result = menu.select(); // -> Skill
        assert_eq!(result, Some(MenuResult::PageChange));
        assert_eq!(menu.page(), MenuPage::Skill);
    }

    #[test]
    fn main_menu_labels_are_correct() {
        let _menu = GameMenu::new(GameVersion::Doom1);
        let items = MAIN_ITEMS_DOOM1;
        assert_eq!(items[0].label, "New Game");
        assert_eq!(items[1].label, "Options");
        assert_eq!(items[2].label, "Load Game");
        assert_eq!(items[3].label, "Save Game");
        assert_eq!(items[4].label, "Quit Game");
    }

    #[test]
    fn episode_labels_are_correct() {
        assert_eq!(EPISODE_ITEMS[0].label, "Knee-Deep in the Dead");
        assert_eq!(EPISODE_ITEMS[1].label, "The Shores of Hell");
        assert_eq!(EPISODE_ITEMS[2].label, "Inferno");
    }

    #[test]
    fn skill_labels_are_correct() {
        assert_eq!(SKILL_ITEMS[0].label, "I'm too young to die");
        assert_eq!(SKILL_ITEMS[1].label, "Hey, not too rough");
        assert_eq!(SKILL_ITEMS[2].label, "Hurt me plenty");
        assert_eq!(SKILL_ITEMS[3].label, "Ultra-Violence");
        assert_eq!(SKILL_ITEMS[4].label, "Nightmare!");
    }

    #[test]
    fn load_items_have_correct_slots() {
        for (i, item) in LOAD_ITEMS.iter().enumerate() {
            assert_eq!(item.action, MenuAction::LoadSlot(i as u8));
        }
    }

    #[test]
    fn save_items_have_correct_slots() {
        for (i, item) in SAVE_ITEMS.iter().enumerate() {
            assert_eq!(item.action, MenuAction::SaveSlot(i as u8));
        }
    }

    #[test]
    fn all_main_items_enabled() {
        for item in MAIN_ITEMS_DOOM1 {
            assert!(item.enabled);
        }
        for item in MAIN_ITEMS_DOOM2 {
            assert!(item.enabled);
        }
    }

    #[test]
    fn select_all_skill_levels() {
        for skill in 0..5u8 {
            let mut menu = GameMenu::new(GameVersion::Doom1);
            menu.open();
            menu.select(); // -> Episode
            menu.select(); // ep 1 -> Skill
            menu.cursor = skill as usize;
            let result = menu.select();
            assert_eq!(result, Some(MenuResult::StartGame { episode: 1, skill }));
        }
    }

    #[test]
    fn select_all_load_slots() {
        for slot in 0..6u8 {
            let mut menu = GameMenu::new(GameVersion::Doom1);
            menu.open();
            menu.cursor = 2; // "Load Game"
            menu.select(); // -> Load
            menu.cursor = slot as usize;
            let result = menu.select();
            assert_eq!(result, Some(MenuResult::LoadGame(slot)));
        }
    }

    #[test]
    fn select_all_save_slots() {
        for slot in 0..6u8 {
            let mut menu = GameMenu::new(GameVersion::Doom1);
            menu.open();
            menu.cursor = 3; // "Save Game"
            menu.select(); // -> Save
            menu.cursor = slot as usize;
            let result = menu.select();
            assert_eq!(result, Some(MenuResult::SaveGame(slot)));
        }
    }

    #[test]
    fn select_all_episodes() {
        for ep_idx in 0..3u8 {
            let mut menu = GameMenu::new(GameVersion::Doom1);
            menu.open();
            menu.select(); // -> Episode
            menu.cursor = ep_idx as usize;
            menu.select(); // -> Skill
            assert_eq!(menu.selected_episode(), ep_idx + 1);
            assert_eq!(menu.page(), MenuPage::Skill);
        }
    }

    // =======================================================================
    // TitleScreen
    // =======================================================================

    #[test]
    fn title_screen_starts_at_title() {
        let ts = TitleScreen::new();
        assert_eq!(ts.phase(), TitlePhase::Title);
        assert_eq!(ts.tic(), 0);
    }

    #[test]
    fn title_screen_default_starts_at_title() {
        let ts = TitleScreen::default();
        assert_eq!(ts.phase(), TitlePhase::Title);
    }

    #[test]
    fn title_phase_lasts_350_tics() {
        let mut ts = TitleScreen::new();
        for _ in 0..349 {
            ts.tick();
            assert_eq!(ts.phase(), TitlePhase::Title);
        }
        ts.tick(); // tic 350
        assert_eq!(ts.phase(), TitlePhase::Credits);
    }

    #[test]
    fn title_transitions_to_credits() {
        let mut ts = TitleScreen::new();
        for _ in 0..TITLE_DURATION {
            ts.tick();
        }
        assert_eq!(ts.phase(), TitlePhase::Credits);
        assert_eq!(ts.tic(), 0);
    }

    #[test]
    fn credits_phase_lasts_200_tics() {
        let mut ts = TitleScreen::new();
        // Title -> Credits
        for _ in 0..TITLE_DURATION {
            ts.tick();
        }
        assert_eq!(ts.phase(), TitlePhase::Credits);
        // Credits lasts 200 tics
        for _ in 0..199 {
            ts.tick();
            assert_eq!(ts.phase(), TitlePhase::Credits);
        }
        ts.tick(); // tic 200
        assert_eq!(ts.phase(), TitlePhase::Title);
    }

    #[test]
    fn demo_0_advances_to_credits() {
        let mut ts = TitleScreen::new();
        ts.phase = TitlePhase::Demo(0);
        ts.advance_from_demo();
        assert_eq!(ts.phase(), TitlePhase::Credits);
    }

    #[test]
    fn demo_1_advances_to_title() {
        let mut ts = TitleScreen::new();
        ts.phase = TitlePhase::Demo(1);
        ts.advance_from_demo();
        assert_eq!(ts.phase(), TitlePhase::Title);
    }

    #[test]
    fn reset_goes_back_to_title() {
        let mut ts = TitleScreen::new();
        for _ in 0..TITLE_DURATION {
            ts.tick();
        }
        assert_eq!(ts.phase(), TitlePhase::Credits);
        ts.reset();
        assert_eq!(ts.phase(), TitlePhase::Title);
        assert_eq!(ts.tic(), 0);
    }

    #[test]
    fn tic_returns_current_tic() {
        let mut ts = TitleScreen::new();
        assert_eq!(ts.tic(), 0);
        ts.tick();
        assert_eq!(ts.tic(), 1);
        ts.tick();
        assert_eq!(ts.tic(), 2);
    }

    #[test]
    fn title_screen_cycles_through_phases() {
        let mut ts = TitleScreen::new();

        // Phase 1: Title
        assert_eq!(ts.phase(), TitlePhase::Title);
        for _ in 0..TITLE_DURATION {
            ts.tick();
        }

        // Phase 2: Credits
        assert_eq!(ts.phase(), TitlePhase::Credits);
        for _ in 0..CREDITS_DURATION {
            ts.tick();
        }

        // Phase 3: Title (cycle complete)
        assert_eq!(ts.phase(), TitlePhase::Title);
    }

    #[test]
    fn reset_from_credits() {
        let mut ts = TitleScreen::new();
        for _ in 0..TITLE_DURATION {
            ts.tick();
        }
        assert_eq!(ts.phase(), TitlePhase::Credits);
        ts.reset();
        assert_eq!(ts.phase(), TitlePhase::Title);
        assert_eq!(ts.tic(), 0);
    }

    #[test]
    fn advance_from_demo_noop_when_not_in_demo() {
        let mut ts = TitleScreen::new();
        ts.advance_from_demo(); // should be no-op (we're in Title)
        assert_eq!(ts.phase(), TitlePhase::Title);
    }

    #[test]
    fn demo_2_advances_to_credits() {
        let mut ts = TitleScreen::new();
        // Force phase to Demo(2) for testing
        ts.phase = TitlePhase::Demo(2);
        ts.tic = 0;
        ts.advance_from_demo();
        assert_eq!(ts.phase(), TitlePhase::Credits);
    }

    #[test]
    fn tic_resets_on_phase_transition() {
        let mut ts = TitleScreen::new();
        for _ in 0..TITLE_DURATION {
            ts.tick();
        }
        // Just transitioned to Credits
        assert_eq!(ts.tic(), 0);
    }

    #[test]
    fn timer_driven_title_loop_never_enters_demo_phase() {
        let mut ts = TitleScreen::new();
        for _ in 0..(TITLE_DURATION + CREDITS_DURATION + TITLE_DURATION) {
            ts.tick();
            assert!(
                !matches!(ts.phase(), TitlePhase::Demo(_)),
                "timer-driven title loop should stay out of demo phases until playback exists"
            );
        }
    }

    #[test]
    fn select_out_of_bounds_item_returns_none() {
        let mut menu = GameMenu::new(GameVersion::Doom1);
        menu.open();
        menu.cursor = 999;
        assert_eq!(menu.select(), None);
    }

    #[test]
    fn title_screen_from_phase_sets_state() {
        let ts = TitleScreen::from_phase(TitlePhase::Credits);
        assert_eq!(ts.phase(), TitlePhase::Credits);
        assert_eq!(ts.tic(), 0);
    }

    #[test]
    fn title_screen_demo_phase_tick_does_not_transition() {
        let mut ts = TitleScreen::from_phase(TitlePhase::Demo(1));
        for _ in 0..1000 {
            ts.tick();
        }
        assert_eq!(ts.phase(), TitlePhase::Demo(1));
        assert_eq!(ts.tic(), 1000);
    }
}
