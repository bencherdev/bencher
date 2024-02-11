use game::play_game;

// FizzBuzz & FizzBuzzFibonacci Game
// fn main() {
//     for i in 1..=100 {
//         play_game(i, true);
//     }
// }

// Open World FizzBuzzFibonacci Game
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n = args
        .get(1)
        .map(|s| s.parse::<u32>())
        .unwrap_or(Ok(15))
        .unwrap_or(15);
    play_game(n, true);
}
