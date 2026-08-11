use game_core::input::{GameAction, MAX_PLAYERS, PlayerActions, TickInputs};

#[test]
fn input_surface_is_available_to_external_tests() {
    let actions = PlayerActions::from_actions([GameAction::Left, GameAction::SoftDrop]);
    let inputs = TickInputs::new([actions, PlayerActions::EMPTY]).expect("two slots fit");

    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs.player(0), Some(actions));
    assert!(TickInputs::new([PlayerActions::EMPTY; MAX_PLAYERS]).is_ok());
}
