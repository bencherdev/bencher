#![feature(test)]
extern crate test;

fn main() {
    play_fizz_buzz();
}

pub fn play_fizz_buzz() {
    for i in 1..=100 {
        println!("{}", fizz_buzz(i));
    }
}

pub fn fizz_buzz(i: i32) -> String {
    match (i % 3, i % 5) {
        (0, 0) => "FizzBuzz".to_string(),
        (0, _) => "Fizz".to_string(),
        (_, 0) => "Buzz".to_string(),
        (_, _) => i.to_string(),
    }
}

#[cfg(test)]
mod benchmarks {
    use test::Bencher;

    use super::play_fizz_buzz;

    #[bench]
    fn bench_play_fizz_buzz(b: &mut Bencher) {
        b.iter(|| play_fizz_buzz());
    }
}
