#![feature(test)]
extern crate test;

// FizzBuzz & FizzBuzzFibonacci Game
// fn main() {
//     for i in 1..=100 {
//         play_game(i);
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
    play_game(n);
}

// FizzBuzz Game
// pub fn play_game(n: u32) {
//     println!("{}", fizz_buzz(n));
// }

// FizzBuzzFibonacci Game
pub fn play_game(n: u32) {
    println!("{}", fizz_buzz_fibonacci(n));
}

pub fn fizz_buzz(n: u32) -> String {
    match (n % 3, n % 5) {
        (0, 0) => "FizzBuzz".to_string(),
        (0, _) => "Fizz".to_string(),
        (_, 0) => "Buzz".to_string(),
        (_, _) => n.to_string(),
    }
}

pub fn fizz_buzz_fibonacci(n: u32) -> String {
    if is_fibonacci_number(n) {
        "Fibonacci".to_string()
    } else {
        match (n % 3, n % 5) {
            (0, 0) => "FizzBuzz".to_string(),
            (0, _) => "Fizz".to_string(),
            (_, 0) => "Buzz".to_string(),
            (_, _) => n.to_string(),
        }
    }
}

// FizzBuzzFibonacci Game
// fn is_fibonacci_number(n: u32) -> bool {
//     for i in 0..=n {
//         let (mut previous, mut current) = (0, 1);
//         while current < i {
//             let next = previous + current;
//             previous = current;
//             current = next;
//         }
//         if current == n {
//             return true;
//         }
//     }
//     false
// }

// Fixed Open World FizzBuzzFibonacci Game
fn is_fibonacci_number(n: u32) -> bool {
    let (mut previous, mut current) = (0, 1);
    while current < n {
        let next = previous + current;
        previous = current;
        current = next;
    }
    current == n
}

#[cfg(test)]
mod benchmarks {
    use test::Bencher;

    use super::play_game;

    #[bench]
    fn bench_play_game(b: &mut Bencher) {
        b.iter(|| {
            std::hint::black_box(for i in 1..=100 {
                play_game(i)
            });
        });
    }

    #[bench]
    fn bench_play_game_100(b: &mut Bencher) {
        b.iter(|| std::hint::black_box(play_game(100)));
    }

    #[bench]
    fn bench_play_game_1_000_000(b: &mut Bencher) {
        b.iter(|| std::hint::black_box(play_game(1_000_000)));
    }
}
