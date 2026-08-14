#![allow(dead_code)]

use game_core::{
    MatchState,
    config::{
        CharacterId, RuleProfileId, ValidatedRuleLibrary, parse_character_play,
        parse_fever_puzzle_book, parse_roster, parse_rule_profile,
    },
    input::{PlayerActions, TickInputs},
    match_spec::{LockedMatchSpec, MatchRequest},
};

const PROFILE: &str = include_str!("../../../../assets/data/rules/profiles/fever.ron");
const ROSTER: &str = include_str!("../../../../assets/data/rules/roster.ron");
const BOOK: &str = include_str!("../../../../assets/data/rules/puzzles/fever-r1.ron");
const PLAY_A: &str = include_str!("../../../../assets/data/rules/play/fever-r1/psi-a.ron");
const PLAY_B: &str = include_str!("../../../../assets/data/rules/play/fever-r1/psi-b.ron");

pub fn library() -> ValidatedRuleLibrary {
    ValidatedRuleLibrary::new(
        vec![parse_rule_profile(PROFILE).expect("profile")],
        parse_roster(ROSTER).expect("roster"),
        vec![
            parse_character_play(PLAY_A).expect("play a"),
            parse_character_play(PLAY_B).expect("play b"),
        ],
        vec![parse_fever_puzzle_book(BOOK).expect("book")],
    )
    .expect("repository data validates")
}

pub fn spec(seed: u64) -> LockedMatchSpec {
    let library = library();
    LockedMatchSpec::freeze(
        MatchRequest {
            rule_profile_id: RuleProfileId("fever-r1".into()),
            root_seed: seed,
            characters: [CharacterId("psi-a".into()), CharacterId("psi-b".into())],
        },
        &library,
    )
    .expect("spec freezes")
}

pub fn state(seed: u64) -> MatchState {
    MatchState::new(spec(seed))
}

pub fn idle() -> TickInputs {
    TickInputs::new([PlayerActions::EMPTY, PlayerActions::EMPTY]).expect("two slots")
}
