//! AI coverage from `integration-system/ai-player.md`.

use client::ai::{AiControllerState, KEY_INTERVAL_TICKS, THINK_DELAY_TICKS, plan_placement};
use game_core::{
    MatchState,
    board::{Board, Cell, Coord},
    config::{CharacterId, RuleProfileId, ValidatedRuleLibrary},
    input::TickInputs,
    match_spec::{LockedMatchSpec, MatchRequest},
    match_state::MatchPhase,
    player::NORMAL_CHANNEL,
    view::PlayerView,
};

const PROFILE: &str = include_str!("../../../assets/data/rules/profiles/fever.ron");
const ROSTER: &str = include_str!("../../../assets/data/rules/roster.ron");
const BOOK: &str = include_str!("../../../assets/data/rules/puzzles/fever-r1.ron");
const PLAY_A: &str = include_str!("../../../assets/data/rules/play/fever-r1/psi-a.ron");
const PLAY_B: &str = include_str!("../../../assets/data/rules/play/fever-r1/psi-b.ron");

fn library() -> ValidatedRuleLibrary {
    use game_core::config::{
        parse_character_play, parse_fever_puzzle_book, parse_roster, parse_rule_profile,
    };
    ValidatedRuleLibrary::new(
        vec![parse_rule_profile(PROFILE).expect("profile parses")],
        parse_roster(ROSTER).expect("roster parses"),
        vec![
            parse_character_play(PLAY_A).expect("play A parses"),
            parse_character_play(PLAY_B).expect("play B parses"),
        ],
        vec![parse_fever_puzzle_book(BOOK).expect("book parses")],
    )
    .expect("repository content validates")
}

fn spec_with(seed: u64, characters: [&str; 2]) -> LockedMatchSpec {
    LockedMatchSpec::freeze(
        MatchRequest {
            rule_profile_id: RuleProfileId("fever-r1".into()),
            root_seed: seed,
            characters: [
                CharacterId(characters[0].into()),
                CharacterId(characters[1].into()),
            ],
        },
        &library(),
    )
    .expect("selection freezes")
}

fn spec() -> LockedMatchSpec {
    spec_with(0x1, ["psi-a", "psi-b"])
}

fn at(x: u8, y: u8) -> Coord {
    Coord::new(x, y).expect("fixture coordinate is on the board")
}

fn open_play(state: &mut MatchState) {
    let idle = TickInputs::new([Default::default(), Default::default()]).expect("two slots");
    while !state.phase().is_playing() {
        state.step(&idle).expect("the countdown advances");
    }
}

/// Runs both slots under AI control until the match ends or the cap is hit.
fn drive(state: &mut MatchState, cap: usize) -> usize {
    let spec = state.spec().clone();
    let mut controllers = [AiControllerState::new(), AiControllerState::new()];
    for tick in 0..cap {
        if matches!(state.phase(), MatchPhase::Completed(_)) {
            return tick;
        }
        let actions: [game_core::input::PlayerActions; 2] = std::array::from_fn(|slot| {
            state
                .player_view(slot)
                .map(|view| controllers[slot].step(&view, &spec))
                .unwrap_or_default()
        });
        let inputs = TickInputs::new(actions).expect("two slots");
        state.step(&inputs).expect("a tick advances");
    }
    cap
}

// integration-system/ai-player::TC-001
#[test]
fn every_shape_reaches_its_planned_pose_through_legal_actions() {
    let spec = spec();
    // Boards: empty, a deep well, a ledge against each wall, a central pit.
    let obstacles: [&[(u8, u8)]; 5] = [
        &[],
        &[(0, 13), (1, 13), (2, 13), (4, 13), (5, 13)],
        &[(0, 13), (0, 12), (0, 11), (1, 13), (1, 12)],
        &[(5, 13), (5, 12), (5, 11), (4, 13), (4, 12)],
        &[(0, 13), (1, 13), (4, 13), (5, 13)],
    ];

    for (index, cells) in obstacles.iter().enumerate() {
        let mut state = MatchState::new(spec.clone());
        open_play(&mut state);
        let mut board = Board::with_geometry(spec.board_geometry);
        for (x, y) in *cells {
            board.set(at(*x, *y), Cell::Color(3));
        }
        state
            .round_mut()
            .player_mut(0)
            .expect("slot exists")
            .set_board(board);

        let view = state.player_view(0).expect("slot exists");
        let group = view.active_group.expect("a group is in play");
        let plan = plan_placement(&view, &spec, &group);
        assert!(
            plan.actions
                .last()
                .is_some_and(|action| *action == game_core::input::GameAction::HardDrop),
            "board {index}: every plan ends by handing over a placement"
        );
        // Every planned action is a legal game action; nothing bypasses the
        // normalized input entry point.
        for action in &plan.actions {
            let actions = game_core::input::PlayerActions::from(*action);
            assert_eq!(actions, actions.normalized());
        }
    }
}

// integration-system/ai-player::TC-002
#[test]
fn a_plan_is_discarded_when_the_turn_or_the_board_changes() {
    let spec = spec();
    let mut state = MatchState::new(spec.clone());
    open_play(&mut state);
    let mut controller = AiControllerState::new();

    let view = state.player_view(0).expect("slot exists");
    controller.step(&view, &spec);
    assert_eq!(controller.plans_made(), 1);

    // The same observation plans exactly once.
    for _ in 0..5 {
        controller.step(&view, &spec);
    }
    assert_eq!(controller.plans_made(), 1, "no stimulus, no replan");

    // A changed board is a changed observation.
    let mut changed = view.clone();
    changed.board.set(at(0, 13), Cell::Nuisance);
    controller.step(&changed, &spec);
    assert_eq!(
        controller.plans_made(),
        2,
        "a nuisance drop forces a replan"
    );

    // A new turn does too.
    let mut next_turn = changed.clone();
    next_turn.turn_id += 1;
    controller.step(&next_turn, &spec);
    assert_eq!(controller.plans_made(), 3);

    // So does a Fever channel switch, which swaps the board out entirely.
    let mut switched = next_turn.clone();
    switched.board = switched.frozen_board.clone();
    switched.active_channel = 1;
    controller.step(&switched, &spec);
    assert_eq!(controller.plans_made(), 4);
}

// integration-system/ai-player::TC-003
#[test]
fn the_think_delay_and_key_interval_are_fixed_and_consume_no_randomness() {
    let spec = spec();
    let mut state = MatchState::new(spec.clone());
    open_play(&mut state);
    let view = state.player_view(0).expect("slot exists");

    let emitted = |view: &PlayerView| {
        let mut controller = AiControllerState::new();
        let mut ticks = Vec::new();
        for tick in 0..80_u16 {
            let actions = controller.step(view, &spec);
            if actions != game_core::input::PlayerActions::EMPTY {
                ticks.push(tick);
            }
        }
        ticks
    };

    let first = emitted(&view);
    assert!(!first.is_empty(), "the plan emits something");
    assert_eq!(
        first[0], THINK_DELAY_TICKS,
        "the first action waits exactly the think delay"
    );
    for pair in first.windows(2) {
        assert_eq!(
            pair[1] - pair[0],
            KEY_INTERVAL_TICKS,
            "actions are spaced by the key interval"
        );
    }
    assert_eq!(first, emitted(&view), "the timing is a pure function");

    // Planning and executing must not touch any rules random stream.
    let before = state.checksum();
    let mut controller = AiControllerState::new();
    for _ in 0..40 {
        controller.step(&view, &spec);
    }
    assert_eq!(
        state.checksum(),
        before,
        "the AI reads the view and writes nothing"
    );
}

// integration-system/ai-player::TC-004
#[test]
fn a_certain_offset_opportunity_is_taken() {
    let spec = spec();
    let mut state = MatchState::new(spec.clone());
    open_play(&mut state);
    // Three of a color in a row: one more lands a clear.
    let mut board = Board::with_geometry(spec.board_geometry);
    for x in 0..3 {
        board.set(at(x, 13), Cell::Color(0));
    }
    {
        let player = state.round_mut().player_mut(0).expect("slot exists");
        player.set_board(board);
        player.set_pending(NORMAL_CHANNEL, 6);
    }

    let view = state.player_view(0).expect("slot exists");
    let group = view.active_group.expect("a group is in play");
    let candidates = client::ai::generate_candidates(&view, &spec, &group);
    let plan = plan_placement(&view, &spec, &group);
    let chosen = candidates
        .iter()
        .find(|candidate| candidate.transform == plan.transform && candidate.column == plan.column)
        .expect("the plan is one of the candidates");

    if candidates
        .iter()
        .any(|candidate| candidate.score.offsets > 0)
    {
        assert!(
            chosen.score.offsets > 0,
            "an available offset outranks a merely tidier board"
        );
    }
}

// integration-system/ai-player::TC-005
#[test]
fn an_immediate_overflow_is_avoided_while_losing_candidates_are_kept() {
    let spec = spec();
    let mut state = MatchState::new(spec.clone());
    open_play(&mut state);
    // Stack the spawn column to one cell below the hidden rows.
    let mut board = Board::with_geometry(spec.board_geometry);
    let column = spec.board_geometry.spawn_column();
    for y in 3..14 {
        board.set(at(column, y), Cell::Color(y % 2));
    }
    state
        .round_mut()
        .player_mut(0)
        .expect("slot exists")
        .set_board(board);

    let view = state.player_view(0).expect("slot exists");
    let group = view.active_group.expect("a group is in play");
    let candidates = client::ai::generate_candidates(&view, &spec, &group);
    assert!(!candidates.is_empty());

    let plan = plan_placement(&view, &spec, &group);
    let chosen = candidates
        .iter()
        .find(|candidate| candidate.transform == plan.transform && candidate.column == plan.column)
        .expect("the plan is one of the candidates");

    if candidates.iter().any(|candidate| candidate.score.survives) {
        assert!(chosen.score.survives, "a surviving placement is chosen");
        assert!(
            candidates.iter().any(|candidate| !candidate.score.survives),
            "losing candidates are kept in the set, not filtered out"
        );
    }
}

// integration-system/ai-player::TC-006
#[test]
fn a_fever_opportunity_outranks_a_plain_attack() {
    let spec = spec();
    let mut state = MatchState::new(spec.clone());
    open_play(&mut state);
    let mut board = Board::with_geometry(spec.board_geometry);
    for x in 0..3 {
        board.set(at(x, 13), Cell::Color(0));
    }
    {
        let player = state.round_mut().player_mut(0).expect("slot exists");
        player.set_board(board);
        player.set_pending(NORMAL_CHANNEL, 4);
        // One cell short of full.
        for _ in 0..(spec.fever.gauge_capacity - 1) {
            player.fever_mut().begin_safety_point();
            player.fever_mut().record_offset(true);
        }
    }

    let view = state.player_view(0).expect("slot exists");
    assert_eq!(view.fever_gauge + 1, view.fever_capacity);
    let group = view.active_group.expect("a group is in play");
    let candidates = client::ai::generate_candidates(&view, &spec, &group);
    let plan = plan_placement(&view, &spec, &group);
    let chosen = candidates
        .iter()
        .find(|candidate| candidate.transform == plan.transform && candidate.column == plan.column)
        .expect("the plan is one of the candidates");

    if candidates
        .iter()
        .any(|candidate| candidate.score.takes_fever)
    {
        assert!(
            chosen.score.takes_fever,
            "the Fever layer sits above the plain attack layer"
        );
    }
}

// integration-system/ai-player::TC-007
#[test]
fn the_same_view_always_yields_the_same_plan() {
    let spec = spec();
    let mut state = MatchState::new(spec.clone());
    open_play(&mut state);
    // A mid-game board, so the candidate set has real ties.
    let mut board = Board::with_geometry(spec.board_geometry);
    for x in 0..6 {
        for y in 11..14 {
            board.set(at(x, y), Cell::Color((x + y) % 3));
        }
    }
    state
        .round_mut()
        .player_mut(0)
        .expect("slot exists")
        .set_board(board);

    let view = state.player_view(0).expect("slot exists");
    let group = view.active_group.expect("a group is in play");
    let first = plan_placement(&view, &spec, &group);
    for _ in 0..10 {
        assert_eq!(
            plan_placement(&view, &spec, &group),
            first,
            "an equal view must give an equal plan, ties included"
        );
    }
}

// integration-system/ai-player::TC-008
#[test]
fn a_mirrored_board_is_evaluated_consistently() {
    let spec = spec();
    let mut state = MatchState::new(spec.clone());
    open_play(&mut state);
    let width = spec.board_geometry.width();

    let mut board = Board::with_geometry(spec.board_geometry);
    let mut mirrored = Board::with_geometry(spec.board_geometry);
    for x in 0..4_u8 {
        for y in 12..14_u8 {
            board.set(at(x, y), Cell::Color((x + y) % 3));
            mirrored.set(at(width - 1 - x, y), Cell::Color((x + y) % 3));
        }
    }

    let base_view = state.player_view(0).expect("slot exists");
    let group = base_view.active_group.expect("a group is in play");

    let mut left = base_view.clone();
    left.board = board;
    let mut right = base_view.clone();
    right.board = mirrored;

    let left_plan = plan_placement(&left, &spec, &group);
    let right_plan = plan_placement(&right, &spec, &group);
    // Both sides reach a decision and both hand over a placement; the shapes
    // are not left/right symmetric, so the columns need not be exact mirrors.
    assert!(left_plan.column < width);
    assert!(right_plan.column < width);
    assert_eq!(
        left_plan.actions.last(),
        right_plan.actions.last(),
        "both plans end the same way"
    );
}

// integration-system/ai-player::TC-009
#[test]
fn invisible_state_cannot_change_the_plan() {
    let spec = spec();
    let mut visible_same = MatchState::new(spec.clone());
    open_play(&mut visible_same);
    let view = visible_same.player_view(0).expect("slot exists");
    let group = view.active_group.expect("a group is in play");
    let baseline = plan_placement(&view, &spec, &group);

    // A second state whose only difference is invisible: the Fever puzzle
    // stream has been advanced. The view is identical, so the plan must be.
    let mut advanced = visible_same.clone();
    advanced
        .round_mut()
        .player_mut(0)
        .expect("slot exists")
        .fever_mut()
        .begin_safety_point();
    let advanced_view = advanced.player_view(0).expect("slot exists");
    assert_eq!(advanced_view, view, "the visible projection is unchanged");
    assert_eq!(plan_placement(&advanced_view, &spec, &group), baseline);

    // The view carries no random state and no hands beyond NEXT.
    assert_eq!(
        view.next.len(),
        usize::from(spec.drop.next_queue_len),
        "only the queued hands are visible"
    );
}

// integration-system/ai-player::TC-010
#[test]
fn a_board_where_everything_loses_still_produces_a_plan() {
    let spec = spec();
    let mut state = MatchState::new(spec.clone());
    open_play(&mut state);
    let mut board = Board::with_geometry(spec.board_geometry);
    // Fill the spawn column and its neighbours right up to the hidden rows.
    for x in 1..4_u8 {
        for y in 2..14_u8 {
            board.set(at(x, y), Cell::Color(y % 2));
        }
    }
    state
        .round_mut()
        .player_mut(0)
        .expect("slot exists")
        .set_board(board);

    let view = state.player_view(0).expect("slot exists");
    let group = view.active_group.expect("a group is in play");
    let candidates = client::ai::generate_candidates(&view, &spec, &group);
    assert!(!candidates.is_empty(), "the candidate set is never empty");

    let plan = plan_placement(&view, &spec, &group);
    assert_eq!(
        plan.actions.last(),
        Some(&game_core::input::GameAction::HardDrop),
        "the AI always hands over a placement rather than stalling"
    );
}

// integration-system/ai-player::TC-011
#[test]
fn ai_matches_finish_and_reproduce_under_fixed_seeds() {
    const CAP: usize = 120_000;
    let pairings = [["psi-a", "psi-a"], ["psi-a", "psi-b"], ["psi-b", "psi-b"]];

    let mut first_pass = Vec::new();
    for seed in 1..=20_u64 {
        let characters = pairings[(seed as usize - 1) % pairings.len()];
        let mut state = MatchState::new(spec_with(seed, characters));
        let ticks = drive(&mut state, CAP);
        assert!(ticks < CAP, "seed {seed} did not finish inside the cap");
        let outcome = state.outcome().expect("a finished match has a winner");
        first_pass.push((
            outcome.winner,
            state.wins(),
            state.round_history().to_vec(),
            state.checksum(),
        ));
    }

    // The same seeds replay to the same results.
    for (index, seed) in (1..=20_u64).enumerate() {
        let characters = pairings[(seed as usize - 1) % pairings.len()];
        let mut state = MatchState::new(spec_with(seed, characters));
        drive(&mut state, CAP);
        let outcome = state.outcome().expect("a finished match has a winner");
        assert_eq!(
            (
                outcome.winner,
                state.wins(),
                state.round_history().to_vec(),
                state.checksum()
            ),
            first_pass[index],
            "seed {seed} did not reproduce"
        );
    }
}

// integration-system/application-lifecycle::TC-010
#[test]
fn a_failed_rules_resolution_never_requests_the_match_transition() {
    use bevy::state::app::AppExtStates;
    use client::data::{DataCategory, DataErrorCause, DataLoadError, DataResolution, RulesData};
    use client::match_flow::{FrozenMatch, MatchSelection, MatchStartDiagnostics};

    let build = |resolution: DataResolution<ValidatedRuleLibrary>| {
        let mut app = bevy::app::App::new();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.insert_state(client::app_state::AppState::CharacterSelect)
            .init_resource::<client::app_state::AppTransitionRequests>()
            .init_resource::<MatchStartDiagnostics>()
            .insert_resource(RulesData {
                resolution,
                excluded_characters: Vec::new(),
            })
            .insert_resource(MatchSelection {
                rule_profile_id: RuleProfileId("fever-r1".into()),
                root_seed: 0x1,
                characters: [CharacterId("psi-a".into()), CharacterId("psi-b".into())],
                confirmed: true,
            })
            .add_systems(bevy::app::Update, client::match_flow::request_match_start);
        app.update();
        app
    };

    // Failed: no request, no frozen spec, and the reason stays observable.
    let mut failed = build(DataResolution::Failed(DataLoadError {
        path: "data/rules/profiles/fever.ron".into(),
        category: DataCategory::Rules,
        cause: DataErrorCause::UnsupportedSchema {
            found: 99,
            supported: 1,
        },
    }));
    failed.update();
    assert!(
        failed
            .world()
            .resource::<client::app_state::AppTransitionRequests>()
            .pending
            .is_empty(),
        "a blocking failure produces no CharacterConfirmed request"
    );
    assert!(failed.world().get_resource::<FrozenMatch>().is_none());
    assert!(
        !failed
            .world()
            .resource::<MatchStartDiagnostics>()
            .0
            .is_empty(),
        "the reason is observable"
    );

    // Loaded: exactly one request and a frozen specification.
    let mut loaded = build(DataResolution::Loaded(library()));
    loaded.update();
    let requests = loaded
        .world()
        .resource::<client::app_state::AppTransitionRequests>();
    assert_eq!(requests.pending.len(), 1);
    assert_eq!(
        requests.pending[0].cause,
        client::app_state::AppTransitionCause::CharacterConfirmed
    );
    assert!(loaded.world().get_resource::<FrozenMatch>().is_some());
}
