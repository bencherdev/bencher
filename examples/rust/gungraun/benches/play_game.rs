use game::play_game;
use gungraun::prelude::*;
use std::hint::black_box;

#[library_benchmark]
fn bench_play_game_100() {
    for i in 1..=100 {
        play_game(black_box(i), black_box(false))
    }
}

#[library_benchmark]
#[benches::play(100, 1_000_000)]
fn bench_play_game(n: u32) {
    play_game(black_box(n), black_box(false));
}

library_benchmark_group!(
    name = bench_play_game_group,
    benchmarks = [bench_play_game_100, bench_play_game]
);

main!(library_benchmark_groups = bench_play_game_group);
