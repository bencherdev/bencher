use bencher_valid::BASE_36;

const BASE: u128 = 36;
const MAX_CHUNK_SIZE: usize = 16;

pub fn encode_base36(input: &str) -> String {
    let mut result = String::new();
    let chars = BASE_36.chars().collect::<Vec<_>>();

    for chunk in input.as_bytes().chunks(MAX_CHUNK_SIZE) {
        let mut num = chunk.iter().fold(0u128, |acc, &c| {
            acc * (u128::from(u8::MAX) + 1) + u128::from(c)
        });

        while num > 0 {
            let remainder = (num % BASE) as usize;
            if let Some(c) = chars.get(remainder) {
                result.push(*c);
            }
            num /= BASE;
        }
    }

    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_base36_empty() {
        assert_eq!(encode_base36(""), "");
    }

    #[test]
    fn test_encode_base36_single_char() {
        assert_eq!(encode_base36("a"), "2p");
    }

    #[test]
    fn test_encode_base36_hello_world() {
        assert_eq!(encode_base36("Hello, World!"), "fg3h7vqw7een6jwwnzmp");
    }

    #[test]
    fn test_encode_base36_max_chunk_size() {
        let max_length_string = "a".repeat(MAX_CHUNK_SIZE);
        assert_eq!(
            encode_base36(&max_length_string),
            "5rjn2eqj5uddxxihjq3vmi99d"
        );
    }

    #[test]
    fn test_encode_base36_double() {
        let max_length_string = "a".repeat(MAX_CHUNK_SIZE * 2);
        assert_eq!(
            encode_base36(&max_length_string),
            "5rjn2eqj5uddxxihjq3vmi99d5rjn2eqj5uddxxihjq3vmi99d"
        );
    }
}
