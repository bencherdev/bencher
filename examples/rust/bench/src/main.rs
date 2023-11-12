#![feature(test)]
extern crate test;

fn main() {
    for i in 1..=100 {
        play_game(i);
    }
}

// FizzBuzz Game
// pub fn play_game() {
//     println!("{}", fizz_buzz(i));
// }

// FizzBuzzFibonacci Game
pub fn play_game(i: u32) {
    println!("{}", fizz_buzz_fibonacci(i));
}

pub fn fizz_buzz(i: u32) -> String {
    match (i % 3, i % 5) {
        (0, 0) => "FizzBuzz".to_string(),
        (0, _) => "Fizz".to_string(),
        (_, 0) => "Buzz".to_string(),
        (_, _) => i.to_string(),
    }
}

pub fn fizz_buzz_fibonacci(i: u32) -> String {
    if is_fibonacci_number(i) {
        "Fibonacci".to_string()
    } else {
        match (i % 3, i % 5) {
            (0, 0) => "FizzBuzz".to_string(),
            (0, _) => "Fizz".to_string(),
            (_, 0) => "Buzz".to_string(),
            (_, _) => i.to_string(),
        }
    }
}

fn is_fibonacci_number(i: u32) -> bool {
    let (mut previous, mut current) = (0, 1);
    while current < i {
        let next = previous + current;
        previous = current;
        current = next;
    }
    current == i
}

#[cfg(test)]
mod benchmarks {
    use test::Bencher;

    use super::play_game;

    #[bench]
    fn bench_play_game(b: &mut Bencher) {
        b.iter(|| {
            for i in 1..=100 {
                play_game(i)
            }
        });
    }
}
