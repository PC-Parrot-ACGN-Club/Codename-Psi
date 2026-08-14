use game_core::{
    board::{Board, Cell},
    falling::{FallingGroup, GroupBall},
    input::{GameAction, PlayerActions},
};

fn pair(board: &Board) -> FallingGroup {
    FallingGroup::new(
        board.coord(2, 1).expect("spawn pivot exists"),
        vec![
            GroupBall {
                dx: 0,
                dy: 0,
                color: 1,
            },
            GroupBall {
                dx: 0,
                dy: -1,
                color: 2,
            },
        ],
        1,
    )
    .expect("pair is valid")
}

#[test]
fn hard_drop_locks_every_ball_atomically_at_the_lowest_position() {
    let mut board = Board::empty();
    let mut group = pair(&board);
    let locked = group
        .apply_actions(&mut board, PlayerActions::from(GameAction::HardDrop))
        .expect("hard drop is valid")
        .expect("hard drop locks");
    assert_eq!(locked.len(), 2);
    assert_eq!(board.get(board.coord(2, 13).unwrap()), Cell::Color(1));
    assert_eq!(board.get(board.coord(2, 12).unwrap()), Cell::Color(2));
}

#[test]
fn blocked_rotation_is_a_no_op_and_never_partially_writes_the_board() {
    let mut board = Board::empty();
    board.set(board.coord(3, 1).unwrap(), Cell::Nuisance);
    let mut group = pair(&board);
    assert!(!group.try_rotate(&board, true));
    assert_eq!(group.pivot(), board.coord(2, 1).unwrap());
    assert_eq!(board.get(board.coord(3, 1).unwrap()), Cell::Nuisance);
}
