#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mean {
    pub mean: f64,
}

impl Mean {
    #[allow(clippy::cast_precision_loss)]
    pub fn new(data: &[f64]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        let mean = data.iter().sum::<f64>() / data.len() as f64;
        mean.is_finite().then_some(Self { mean })
    }

    pub fn std_deviation(self, data: &[f64]) -> Option<f64> {
        std_deviation(self.mean, data)
    }
}

pub fn std_deviation(location: f64, data: &[f64]) -> Option<f64> {
    variance(location, data)
    // If the variance is zero then the standard deviation is not going to work with `statrs`
        .and_then(|std_dev| if std_dev == 0.0 { None } else { Some(std_dev) })
        .map(f64::sqrt)
        .and_then(|m| m.is_finite().then_some(m))
}

fn variance(location: f64, data: &[f64]) -> Option<f64> {
    // Do not calculate variance if there are less than 2 data points
    if data.len() < 2 {
        None
    } else {
        #[allow(clippy::cast_precision_loss)]
        Some(
            data.iter()
                .map(|&value| (value - location).powi(2))
                .sum::<f64>()
                / data.len() as f64,
        )
        .and_then(|v| v.is_finite().then_some(v))
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::unreadable_literal, clippy::unwrap_used)]
mod test {
    use once_cell::sync::Lazy;
    use pretty_assertions::assert_eq;

    use crate::mean::variance;

    use super::Mean;

    const DATA_ZERO: &[f64] = &[];
    const DATA_ONE: &[f64] = &[1.0];
    const DATA_TWO: &[f64] = &[1.0, 2.0];
    const DATA_THREE: &[f64] = &[1.0, 2.0, 3.0];
    const DATA_FIVE: &[f64] = &[1.0, 2.0, 3.0, 4.0, 5.0];
    const DATA_FIVE_DESC: &[f64] = &[5.0, 4.0, 3.0, 2.0, 1.0];
    const DATA_FIVE_NEG: &[f64] = &[-1.0, -2.0, -3.0, -4.0, -5.0];
    const DATA_FIVE_CONST: &[f64] = &[1.0, 1.0, 1.0, 1.0, 1.0];

    // const MEAN_ZERO: f64 = 0.0;
    // const MEAN_ONE: f64 = 1.0;
    // const MEAN_TWO: f64 = 1.5;
    // const MEAN_THREE: f64 = 2.0;
    // const MEAN_FIVE: f64 = 3.0;
    // const MEAN_SIX_BI: f64 = 0.3333333333333333;

    static MEAN_ZERO: Lazy<Mean> = Lazy::new(|| Mean { mean: 0.0 });
    static MEAN_ONE: Lazy<Mean> = Lazy::new(|| Mean { mean: 1.0 });
    static MEAN_TWO: Lazy<Mean> = Lazy::new(|| Mean { mean: 1.5 });
    static MEAN_THREE: Lazy<Mean> = Lazy::new(|| Mean { mean: 2.0 });
    static MEAN_FIVE: Lazy<Mean> = Lazy::new(|| Mean { mean: 3.0 });

    static MEAN_NEG_ONE: Lazy<Mean> = Lazy::new(|| Mean { mean: -1.0 });
    static MEAN_NEG_TWO: Lazy<Mean> = Lazy::new(|| Mean { mean: -1.5 });
    static MEAN_NEG_THREE: Lazy<Mean> = Lazy::new(|| Mean { mean: -2.0 });
    static MEAN_NEG_FIVE: Lazy<Mean> = Lazy::new(|| Mean { mean: -3.0 });

    #[test]
    fn test_mean_zero() {
        let m = Mean::new(DATA_ZERO);
        assert_eq!(m, None);
    }

    #[test]
    fn test_mean_one() {
        let m = Mean::new(DATA_ONE).unwrap();
        assert_eq!(m, *MEAN_ONE);
    }

    #[test]
    fn test_mean_two() {
        let m = Mean::new(DATA_TWO).unwrap();
        assert_eq!(m, *MEAN_TWO);
    }

    #[test]
    fn test_mean_three() {
        let m = Mean::new(DATA_THREE).unwrap();
        assert_eq!(m, *MEAN_THREE);
    }

    #[test]
    fn test_mean_five() {
        let m = Mean::new(DATA_FIVE).unwrap();
        assert_eq!(m, *MEAN_FIVE);
    }

    #[test]
    fn test_mean_five_desc() {
        let m = Mean::new(DATA_FIVE_DESC).unwrap();
        assert_eq!(m, *MEAN_FIVE);
    }

    #[test]
    fn test_mean_five_neg() {
        let m = Mean::new(DATA_FIVE_NEG).unwrap();
        assert_eq!(m, Mean { mean: -3.0 });
    }

    #[test]
    fn test_mean_five_const() {
        let m = Mean::new(DATA_FIVE_CONST).unwrap();
        assert_eq!(m, *MEAN_ONE);
    }

    #[test]
    fn test_variance_zero() {
        let v = variance(MEAN_ZERO.mean, DATA_ZERO);
        assert_eq!(v, None);

        let v = variance(MEAN_ONE.mean, DATA_ZERO);
        assert_eq!(v, None);

        let v = variance(MEAN_TWO.mean, DATA_ZERO);
        assert_eq!(v, None);

        let v = variance(MEAN_THREE.mean, DATA_ZERO);
        assert_eq!(v, None);

        let v = variance(MEAN_FIVE.mean, DATA_ZERO);
        assert_eq!(v, None);
    }

    #[test]
    fn test_variance_one() {
        let v = variance(MEAN_ZERO.mean, DATA_ONE);
        assert_eq!(v, None);

        let v = variance(MEAN_ONE.mean, DATA_ONE);
        assert_eq!(v, None);

        let v = variance(MEAN_TWO.mean, DATA_ONE);
        assert_eq!(v, None);

        let v = variance(MEAN_THREE.mean, DATA_ONE);
        assert_eq!(v, None);

        let v = variance(MEAN_FIVE.mean, DATA_ONE);
        assert_eq!(v, None);
    }

    #[test]
    fn test_variance_two() {
        let v = variance(MEAN_ZERO.mean, DATA_TWO).unwrap();
        assert_eq!(v, 2.5);

        let v = variance(MEAN_ONE.mean, DATA_TWO).unwrap();
        assert_eq!(v, 0.5);

        let v = variance(MEAN_TWO.mean, DATA_TWO).unwrap();
        assert_eq!(v, 0.25);

        let v = variance(MEAN_THREE.mean, DATA_TWO).unwrap();
        assert_eq!(v, 0.5);

        let v = variance(MEAN_FIVE.mean, DATA_TWO).unwrap();
        assert_eq!(v, 2.5);
    }

    #[test]
    fn test_variance_three() {
        let v = variance(MEAN_ZERO.mean, DATA_THREE).unwrap();
        assert_eq!(v, 4.666666666666667);

        let v = variance(MEAN_ONE.mean, DATA_THREE).unwrap();
        assert_eq!(v, 1.6666666666666667);

        let v = variance(MEAN_TWO.mean, DATA_THREE).unwrap();
        assert_eq!(v, 0.9166666666666666);

        let v = variance(MEAN_THREE.mean, DATA_THREE).unwrap();
        assert_eq!(v, 0.6666666666666666);

        let v = variance(MEAN_FIVE.mean, DATA_THREE).unwrap();
        assert_eq!(v, 1.6666666666666667);
    }

    #[test]
    fn test_variance_five() {
        let v = variance(MEAN_ZERO.mean, DATA_FIVE).unwrap();
        assert_eq!(v, 11.0);

        let v = variance(MEAN_ONE.mean, DATA_FIVE).unwrap();
        assert_eq!(v, 6.0);

        let v = variance(MEAN_TWO.mean, DATA_FIVE).unwrap();
        assert_eq!(v, 4.25);

        let v = variance(MEAN_THREE.mean, DATA_FIVE).unwrap();
        assert_eq!(v, 3.0);

        let v = variance(MEAN_FIVE.mean, DATA_FIVE).unwrap();
        assert_eq!(v, 2.0);
    }

    #[test]
    fn test_variance_five_desc() {
        let v = variance(MEAN_ZERO.mean, DATA_FIVE_DESC).unwrap();
        assert_eq!(v, 11.0);

        let v = variance(MEAN_ONE.mean, DATA_FIVE_DESC).unwrap();
        assert_eq!(v, 6.0);

        let v = variance(MEAN_TWO.mean, DATA_FIVE_DESC).unwrap();
        assert_eq!(v, 4.25);

        let v = variance(MEAN_THREE.mean, DATA_FIVE_DESC).unwrap();
        assert_eq!(v, 3.0);

        let v = variance(MEAN_FIVE.mean, DATA_FIVE_DESC).unwrap();
        assert_eq!(v, 2.0);
    }

    #[test]
    fn test_variance_five_neg() {
        let v = variance(MEAN_ZERO.mean, DATA_FIVE_NEG).unwrap();
        assert_eq!(v, 11.0);

        let v = variance(MEAN_NEG_ONE.mean, DATA_FIVE_NEG).unwrap();
        assert_eq!(v, 6.0);

        let v = variance(MEAN_NEG_TWO.mean, DATA_FIVE_NEG).unwrap();
        assert_eq!(v, 4.25);

        let v = variance(MEAN_NEG_THREE.mean, DATA_FIVE_NEG).unwrap();
        assert_eq!(v, 3.0);

        let v = variance(MEAN_NEG_FIVE.mean, DATA_FIVE_NEG).unwrap();
        assert_eq!(v, 2.0);
    }

    #[test]
    fn test_variance_five_const() {
        let v = variance(MEAN_ZERO.mean, DATA_FIVE_CONST).unwrap();
        assert_eq!(v, 1.0);

        let v = variance(MEAN_ONE.mean, DATA_FIVE_CONST).unwrap();
        assert_eq!(v, 0.0);

        let v = variance(MEAN_TWO.mean, DATA_FIVE_CONST).unwrap();
        assert_eq!(v, 0.25);

        let v = variance(MEAN_THREE.mean, DATA_FIVE_CONST).unwrap();
        assert_eq!(v, 1.0);

        let v = variance(MEAN_FIVE.mean, DATA_FIVE_CONST).unwrap();
        assert_eq!(v, 4.0);
    }

    #[test]
    fn test_std_dev_zero() {
        let std_dev = MEAN_ZERO.std_deviation(DATA_ZERO);
        assert_eq!(std_dev, None);

        let std_dev = MEAN_ONE.std_deviation(DATA_ZERO);
        assert_eq!(std_dev, None);

        let std_dev = MEAN_TWO.std_deviation(DATA_ZERO);
        assert_eq!(std_dev, None);

        let std_dev = MEAN_THREE.std_deviation(DATA_ZERO);
        assert_eq!(std_dev, None);

        let std_dev = MEAN_FIVE.std_deviation(DATA_ZERO);
        assert_eq!(std_dev, None);
    }

    #[test]
    fn test_std_dev_one() {
        let std_dev = MEAN_ZERO.std_deviation(DATA_ONE);
        assert_eq!(std_dev, None);

        let std_dev = MEAN_ONE.std_deviation(DATA_ONE);
        assert_eq!(std_dev, None);

        let std_dev = MEAN_TWO.std_deviation(DATA_ONE);
        assert_eq!(std_dev, None);

        let std_dev = MEAN_THREE.std_deviation(DATA_ONE);
        assert_eq!(std_dev, None);

        let std_dev = MEAN_FIVE.std_deviation(DATA_ONE);
        assert_eq!(std_dev, None);
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_std_dev_two() {
        let std_dev = MEAN_ZERO.std_deviation(DATA_TWO).unwrap();
        assert_eq!(std_dev, 1.5811388300841898);

        let std_dev = MEAN_ONE.std_deviation(DATA_TWO).unwrap();
        assert_eq!(std_dev, 0.7071067811865476);

        let std_dev = MEAN_TWO.std_deviation(DATA_TWO).unwrap();
        assert_eq!(std_dev, 0.5);

        let std_dev = MEAN_THREE.std_deviation(DATA_TWO).unwrap();
        assert_eq!(std_dev, 0.7071067811865476);

        let std_dev = MEAN_FIVE.std_deviation(DATA_TWO).unwrap();
        assert_eq!(std_dev, 1.5811388300841898);
    }

    #[test]
    fn test_std_dev_three() {
        let std_dev = MEAN_ZERO.std_deviation(DATA_THREE).unwrap();
        assert_eq!(std_dev, 2.160246899469287);

        let std_dev = MEAN_ONE.std_deviation(DATA_THREE).unwrap();
        assert_eq!(std_dev, 1.2909944487358056);

        let std_dev = MEAN_TWO.std_deviation(DATA_THREE).unwrap();
        assert_eq!(std_dev, 0.9574271077563381);

        let std_dev = MEAN_THREE.std_deviation(DATA_THREE).unwrap();
        assert_eq!(std_dev, 0.816496580927726);

        let std_dev = MEAN_FIVE.std_deviation(DATA_THREE).unwrap();
        assert_eq!(std_dev, 1.2909944487358056);
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_std_dev_five() {
        let std_dev = MEAN_ZERO.std_deviation(DATA_FIVE).unwrap();
        assert_eq!(std_dev, 3.3166247903554);

        let std_dev = MEAN_ONE.std_deviation(DATA_FIVE).unwrap();
        assert_eq!(std_dev, 2.449489742783178);

        let std_dev = MEAN_TWO.std_deviation(DATA_FIVE).unwrap();
        assert_eq!(std_dev, 2.0615528128088303);

        let std_dev = MEAN_THREE.std_deviation(DATA_FIVE).unwrap();
        assert_eq!(std_dev, 1.7320508075688772);

        let std_dev = MEAN_FIVE.std_deviation(DATA_FIVE).unwrap();
        assert_eq!(std_dev, 1.4142135623730951);
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_std_dev_five_desc() {
        let std_dev = MEAN_ZERO.std_deviation(DATA_FIVE_DESC).unwrap();
        assert_eq!(std_dev, 3.3166247903554);

        let std_dev = MEAN_ONE.std_deviation(DATA_FIVE_DESC).unwrap();
        assert_eq!(std_dev, 2.449489742783178);

        let std_dev = MEAN_TWO.std_deviation(DATA_FIVE_DESC).unwrap();
        assert_eq!(std_dev, 2.0615528128088303);

        let std_dev = MEAN_THREE.std_deviation(DATA_FIVE_DESC).unwrap();
        assert_eq!(std_dev, 1.7320508075688772);

        let std_dev = MEAN_FIVE.std_deviation(DATA_FIVE_DESC).unwrap();
        assert_eq!(std_dev, 1.4142135623730951);
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_std_dev_five_neg() {
        let std_dev = MEAN_ZERO.std_deviation(DATA_FIVE_NEG).unwrap();
        assert_eq!(std_dev, 3.3166247903554);

        let std_dev = MEAN_NEG_ONE.std_deviation(DATA_FIVE_NEG).unwrap();
        assert_eq!(std_dev, 2.449489742783178);

        let std_dev = MEAN_NEG_TWO.std_deviation(DATA_FIVE_NEG).unwrap();
        assert_eq!(std_dev, 2.0615528128088303);

        let std_dev = MEAN_NEG_THREE.std_deviation(DATA_FIVE_NEG).unwrap();
        assert_eq!(std_dev, 1.7320508075688772);

        let std_dev = MEAN_NEG_FIVE.std_deviation(DATA_FIVE_NEG).unwrap();
        assert_eq!(std_dev, 1.4142135623730951);
    }

    #[test]
    fn test_std_dev_five_const() {
        let std_dev = MEAN_ZERO.std_deviation(DATA_FIVE_CONST).unwrap();
        assert_eq!(std_dev, 1.0);

        let std_dev = MEAN_ONE.std_deviation(DATA_FIVE_CONST);
        assert_eq!(std_dev, None);

        let std_dev = MEAN_TWO.std_deviation(DATA_FIVE_CONST).unwrap();
        assert_eq!(std_dev, 0.5);

        let std_dev = MEAN_THREE.std_deviation(DATA_FIVE_CONST).unwrap();
        assert_eq!(std_dev, 1.0);

        let std_dev = MEAN_FIVE.std_deviation(DATA_FIVE_CONST).unwrap();
        assert_eq!(std_dev, 2.0);
    }
}
