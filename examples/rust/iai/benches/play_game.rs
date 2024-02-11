use game::play_game;

fn bench_play_game() {
    iai::black_box(for i in 1..=100 {
        play_game(i, false)
    });
}

fn bench_play_game_100() {
    iai::black_box(play_game(100, false));
}

fn bench_play_game_1_000_000() {
    iai::black_box(play_game(1_000_000, false));
}

iai::main!(
    bench_play_game,
    bench_play_game_100,
    bench_play_game_1_000_000
);
