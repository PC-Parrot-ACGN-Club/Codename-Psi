use game_core::{
    config::{
        CharacterId, CharacterPlay, DropSet, RuleProfileId, ValidatedRuleLibrary,
        parse_rule_profile,
    },
    match_spec::{LockedMatchSpec, MatchRequest},
};

const PROFILE: &str = include_str!("../../../assets/data/rules/profiles/fever.ron");

fn table(value: u16) -> Vec<u16> {
    vec![value; 24]
}

#[test]
fn repository_profile_validates_and_exposes_configured_geometry() {
    let profile = parse_rule_profile(PROFILE).expect("repository profile is valid");
    let geometry = profile.field.geometry().expect("profile geometry is valid");
    assert_eq!(
        (geometry.width(), geometry.height(), geometry.hidden_rows()),
        (6, 14, 2)
    );
    assert_eq!(profile.resolve.clear_preview_ticks, 12);
}

#[test]
fn a_match_freezes_character_curves_and_never_needs_to_read_assets_again() {
    let profile = parse_rule_profile(PROFILE).expect("profile parses");
    let profile_id = profile.id.clone();
    let a = CharacterId("a".into());
    let b = CharacterId("b".into());
    let library = ValidatedRuleLibrary::new(
        vec![profile],
        vec![
            CharacterPlay {
                schema_version: 1,
                profile_id: profile_id.clone(),
                character_id: a.clone(),
                drop_set: DropSet::default(),
                normal_chain_power: table(10),
                fever_chain_power: table(20),
            },
            CharacterPlay {
                schema_version: 1,
                profile_id: profile_id.clone(),
                character_id: b.clone(),
                drop_set: DropSet::default(),
                normal_chain_power: table(30),
                fever_chain_power: table(40),
            },
        ],
    )
    .expect("complete library validates");
    let spec = LockedMatchSpec::freeze(
        MatchRequest {
            rule_profile_id: profile_id,
            root_seed: 7,
            characters: [a, b],
        },
        &library,
    )
    .expect("complete selection freezes");
    assert_eq!(spec.chain_power[0].normal()[0], 10);
    assert_eq!(spec.chain_power[1].fever()[0], 40);
}

#[test]
fn unknown_profile_is_not_frozen_into_a_match() {
    let library =
        ValidatedRuleLibrary::new(vec![], vec![]).expect("empty library is structurally valid");
    let error = LockedMatchSpec::freeze(
        MatchRequest {
            rule_profile_id: RuleProfileId("missing".into()),
            root_seed: 0,
            characters: [CharacterId("a".into()), CharacterId("b".into())],
        },
        &library,
    )
    .expect_err("unknown profile must block match construction");
    assert!(error.to_string().contains("unknown rule profile"));
}
