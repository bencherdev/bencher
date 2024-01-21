use crate::mean::std_deviation;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ln {
    pub location: f64,
    pub scale: f64,
}

impl Ln {
    pub fn new(data: &[f64]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        // https://towardsdatascience.com/log-normal-distribution-a-simple-explanation-7605864fb67c
        let log_data = data.iter().copied().map(f64::ln).collect::<Vec<_>>();
        #[allow(clippy::cast_precision_loss)]
        let location = log_data.iter().copied().sum::<f64>() / log_data.len() as f64;
        if !location.is_finite() {
            return None;
        }
        let scale = std_deviation(location, &log_data)?;
        scale.is_finite().then_some(Self { location, scale })
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::unreadable_literal, clippy::unwrap_used)]
mod test {
    use pretty_assertions::assert_eq;

    use super::Ln;

    const DATA_ZERO: &[f64] = &[];
    const DATA_ONE: &[f64] = &[1.0];
    const DATA_TWO: &[f64] = &[1.0, 2.0];
    const DATA_THREE: &[f64] = &[1.0, 2.0, 3.0];
    const DATA_FIVE: &[f64] = &[1.0, 2.0, 3.0, 4.0, 5.0];
    const DATA_FIVE_DESC: &[f64] = &[5.0, 4.0, 3.0, 2.0, 1.0];
    const DATA_FIVE_NEG: &[f64] = &[-1.0, -2.0, -3.0, -4.0, -5.0];
    const DATA_FIVE_CONST: &[f64] = &[1.0, 1.0, 1.0, 1.0, 1.0];

    #[test]
    fn test_ln_zero() {
        let ln = Ln::new(DATA_ZERO);
        assert_eq!(ln, None);
    }

    #[test]
    fn test_ln_one() {
        let ln = Ln::new(DATA_ONE);
        assert_eq!(ln, None);
    }

    #[test]
    fn test_ln_two() {
        let ln = Ln::new(DATA_TWO).unwrap();
        assert_eq!(
            ln,
            Ln {
                location: 0.34657359027997264,
                scale: 0.34657359027997264
            }
        );
    }

    #[test]
    fn test_ln_three() {
        let ln = Ln::new(DATA_THREE).unwrap();
        assert_eq!(
            ln,
            Ln {
                location: 0.5972531564093516,
                scale: 0.4536033422157818,
            }
        );
    }

    #[test]
    fn test_ln_five() {
        let ln = Ln::new(DATA_FIVE).unwrap();
        assert_eq!(
            ln,
            Ln {
                location: 0.9574983485564091,
                scale: 0.5684169221517898,
            }
        );
    }

    #[test]
    fn test_ln_five_desc() {
        let ln = Ln::new(DATA_FIVE_DESC).unwrap();
        assert_eq!(
            ln,
            Ln {
                location: 0.9574983485564091,
                scale: 0.5684169221517898,
            }
        );
    }

    #[test]
    fn test_ln_five_neg() {
        let ln = Ln::new(DATA_FIVE_NEG);
        assert_eq!(ln, None);
    }

    #[test]
    fn test_ln_five_const() {
        let ln = Ln::new(DATA_FIVE_CONST);
        assert_eq!(ln, None);
    }
}
