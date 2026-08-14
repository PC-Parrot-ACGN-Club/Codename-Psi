use game_core::{
    MatchState,
    config::{CharacterId, CharacterPlay, DropSet, ValidatedRuleLibrary, parse_rule_profile},
    input::{GameAction, PlayerActions, TickInputs},
    match_spec::{LockedMatchSpec, MatchRequest},
    match_state::{MatchPhase, MatchStepError},
};

const PROFILE: &str = include_str!("../../../assets/data/rules/profiles/fever.ron");

fn state() -> MatchState {
    let profile = parse_rule_profile(PROFILE).expect("profile parses");
    let a = CharacterId("a".into());
    let b = CharacterId("b".into());
    let powers = |value| vec![value; 24];
    let library = ValidatedRuleLibrary::new(
        vec![profile.clone()],
        vec![
            CharacterPlay {
                schema_version: 1,
                profile_id: profile.id.clone(),
                character_id: a.clone(),
                drop_set: DropSet::default(),
                normal_chain_power: powers(1),
                fever_chain_power: powers(1),
            },
            CharacterPlay {
                schema_version: 1,
                profile_id: profile.id.clone(),
                character_id: b.clone(),
                drop_set: DropSet::default(),
                normal_chain_power: powers(1),
                fever_chain_power: powers(1),
            },
        ],
    )
    .expect("library validates");
    let spec = LockedMatchSpec::freeze(
        MatchRequest {
            rule_profile_id: profile.id,
            root_seed: 9,
            characters: [a, b],
        },
        &library,
    )
    .expect("spec freezes");
    MatchState::new(spec)
}

#[test]
fn a_locked_group_waits_for_settlement_before_the_next_group_spawns() {
    let mut match_state = state();
    let idle = TickInputs::new(&[PlayerActions::EMPTY, PlayerActions::EMPTY]).unwrap();
    match_state.step(&idle).unwrap();
    let hard_drop = TickInputs::new(&[
        PlayerActions::from(GameAction::HardDrop),
        PlayerActions::EMPTY,
    ])
    .unwrap();
    match_state.step(&hard_drop).unwrap();
    assert!(
        match_state.active_group(0).is_none(),
        "settlement owns the field after a lock"
    );
    match_state.step(&idle).unwrap();
    assert!(
        match_state.active_group(0).is_some(),
        "the next group appears only after settlement"
    );
}

#[test]
fn one_tick_requires_exactly_two_slots_and_then_enters_playing() {
    let mut match_state = state();
    let error = match_state
        .step(&TickInputs::EMPTY)
        .expect_err("one slot count is invalid");
    assert_eq!(
        error,
        MatchStepError::ParticipantCount {
            expected: 2,
            actual: 0
        }
    );
    assert_eq!(
        match_state.match_tick(),
        0,
        "rejection must not mutate state"
    );

    let inputs =
        TickInputs::new(&[PlayerActions::EMPTY, PlayerActions::EMPTY]).expect("two inputs fit");
    let report = match_state
        .step(&inputs)
        .expect("two slots advance the match");
    assert_eq!(report.phase, MatchPhase::Playing);
    assert_eq!(match_state.match_tick(), 1);
}
