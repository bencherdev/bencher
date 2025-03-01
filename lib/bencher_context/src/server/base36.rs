use bencher_valid::BASE_36;
use uuid::Uuid;

pub fn encode_uuid(uuid: Uuid) -> String {
    let base = BASE_36.len() as u128;
    let chars = BASE_36.chars().collect::<Vec<_>>();

    let mut num = uuid.as_u128();
    let mut result = String::new();
    while num > 0 {
        let remainder = (num % base) as usize;
        if let Some(c) = chars.get(remainder) {
            result.push(*c);
        }
        num /= base;
    }

    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_encode_uuid() {
        let uuid = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        assert_eq!(encode_uuid(uuid), "");

        let uuid = Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap();
        assert_eq!(encode_uuid(uuid), "f5lxx1zz5pnorynqglhzmsp33");

        let uuid = Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();
        assert_eq!(encode_uuid(uuid), "12srddy53kndus0lmbgjgy7i0");
    }
}
