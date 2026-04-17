#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mean {
    pub mean: f64,
}

impl Mean {
    pub fn new(data: &[f64]) -> Option<Self> {
        mean(data).map(|mean| Self { mean })
    }

    pub fn std_deviation(self, data: &[f64]) -> Option<f64> {
        std_deviation(self.mean, data)
    }
}

pub fn mean(data: &[f64]) -> Option<f64> {
    if data.is_empty() {
        None
    } else {
        #[expect(clippy::cast_precision_loss)]
        let mean = data.iter().sum::<f64>() / data.len() as f64;
        mean.is_finite().then_some(mean)
    }
}

pub fn std_deviation(location: f64, data: &[f64]) -> Option<f64> {
    variance(location, data)
        // If the variance is zero then the standard deviation is not going to work with `statrs`
        .and_then(|variance| if variance == 0.0 { None } else { Some(variance) })
        .map(f64::sqrt)
        .and_then(|std_dev| std_dev.is_finite().then_some(std_dev))
}

fn variance(location: f64, data: &[f64]) -> Option<f64> {
    // Do not calculate variance if there are less than 2 data points
    if data.len() < 2 {
        None
    } else {
        #[expect(clippy::cast_precision_loss)]
        Some(
            data.iter()
                .map(|&value| (value - location).powi(2))
                .sum::<f64>()
                / (data.len() - 1) as f64,
        )
        .and_then(|v| v.is_finite().then_some(v))
    }
}

#[cfg(test)]
#[expect(clippy::float_cmp, clippy::unreadable_literal)]
mod tests {
    use std::sync::LazyLock;

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

    static MEAN_ZERO: LazyLock<Mean> = LazyLock::new(|| Mean { mean: 0.0 });
    static MEAN_ONE: LazyLock<Mean> = LazyLock::new(|| Mean { mean: 1.0 });
    static MEAN_TWO: LazyLock<Mean> = LazyLock::new(|| Mean { mean: 1.5 });
    static MEAN_THREE: LazyLock<Mean> = LazyLock::new(|| Mean { mean: 2.0 });
    static MEAN_FIVE: LazyLock<Mean> = LazyLock::new(|| Mean { mean: 3.0 });

    static MEAN_NEG_ONE: LazyLock<Mean> = LazyLock::new(|| Mean { mean: -1.0 });
    static MEAN_NEG_TWO: LazyLock<Mean> = LazyLock::new(|| Mean { mean: -1.5 });
    static MEAN_NEG_THREE: LazyLock<Mean> = LazyLock::new(|| Mean { mean: -2.0 });
    static MEAN_NEG_FIVE: LazyLock<Mean> = LazyLock::new(|| Mean { mean: -3.0 });

    #[test]
    fn mean_zero() {
        let m = Mean::new(DATA_ZERO);
        assert_eq!(m, None);
    }

    #[test]
    fn mean_one() {
        let m = Mean::new(DATA_ONE).unwrap();
        assert_eq!(m, *MEAN_ONE);
    }

    #[test]
    fn mean_two() {
        let m = Mean::new(DATA_TWO).unwrap();
        assert_eq!(m, *MEAN_TWO);
    }

    #[test]
    fn mean_three() {
        let m = Mean::new(DATA_THREE).unwrap();
        assert_eq!(m, *MEAN_THREE);
    }

    #[test]
    fn mean_five() {
        let m = Mean::new(DATA_FIVE).unwrap();
        assert_eq!(m, *MEAN_FIVE);
    }

    #[test]
    fn mean_five_desc() {
        let m = Mean::new(DATA_FIVE_DESC).unwrap();
        assert_eq!(m, *MEAN_FIVE);
    }

    #[test]
    fn mean_five_neg() {
        let m = Mean::new(DATA_FIVE_NEG).unwrap();
        assert_eq!(m, Mean { mean: -3.0 });
    }

    #[test]
    fn mean_five_const() {
        let m = Mean::new(DATA_FIVE_CONST).unwrap();
        assert_eq!(m, *MEAN_ONE);
    }

    #[test]
    fn variance_zero() {
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
    fn variance_one() {
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
    fn variance_two() {
        let v = variance(MEAN_ZERO.mean, DATA_TWO).unwrap();
        assert_eq!(v, 5.0);

        let v = variance(MEAN_ONE.mean, DATA_TWO).unwrap();
        assert_eq!(v, 1.0);

        let v = variance(MEAN_TWO.mean, DATA_TWO).unwrap();
        assert_eq!(v, 0.5);

        let v = variance(MEAN_THREE.mean, DATA_TWO).unwrap();
        assert_eq!(v, 1.0);

        let v = variance(MEAN_FIVE.mean, DATA_TWO).unwrap();
        assert_eq!(v, 5.0);
    }

    #[test]
    fn variance_three() {
        let v = variance(MEAN_ZERO.mean, DATA_THREE).unwrap();
        assert_eq!(v, 7.0);

        let v = variance(MEAN_ONE.mean, DATA_THREE).unwrap();
        assert_eq!(v, 2.5);

        let v = variance(MEAN_TWO.mean, DATA_THREE).unwrap();
        assert_eq!(v, 1.375);

        let v = variance(MEAN_THREE.mean, DATA_THREE).unwrap();
        assert_eq!(v, 1.0);

        let v = variance(MEAN_FIVE.mean, DATA_THREE).unwrap();
        assert_eq!(v, 2.5);
    }

    #[test]
    fn variance_five() {
        let v = variance(MEAN_ZERO.mean, DATA_FIVE).unwrap();
        assert_eq!(v, 13.75);

        let v = variance(MEAN_ONE.mean, DATA_FIVE).unwrap();
        assert_eq!(v, 7.5);

        let v = variance(MEAN_TWO.mean, DATA_FIVE).unwrap();
        assert_eq!(v, 5.3125);

        let v = variance(MEAN_THREE.mean, DATA_FIVE).unwrap();
        assert_eq!(v, 3.75);

        let v = variance(MEAN_FIVE.mean, DATA_FIVE).unwrap();
        assert_eq!(v, 2.5);
    }

    #[test]
    fn variance_five_desc() {
        let v = variance(MEAN_ZERO.mean, DATA_FIVE_DESC).unwrap();
        assert_eq!(v, 13.75);

        let v = variance(MEAN_ONE.mean, DATA_FIVE_DESC).unwrap();
        assert_eq!(v, 7.5);

        let v = variance(MEAN_TWO.mean, DATA_FIVE_DESC).unwrap();
        assert_eq!(v, 5.3125);

        let v = variance(MEAN_THREE.mean, DATA_FIVE_DESC).unwrap();
        assert_eq!(v, 3.75);

        let v = variance(MEAN_FIVE.mean, DATA_FIVE_DESC).unwrap();
        assert_eq!(v, 2.5);
    }

    #[test]
    fn variance_five_neg() {
        let v = variance(MEAN_ZERO.mean, DATA_FIVE_NEG).unwrap();
        assert_eq!(v, 13.75);

        let v = variance(MEAN_NEG_ONE.mean, DATA_FIVE_NEG).unwrap();
        assert_eq!(v, 7.5);

        let v = variance(MEAN_NEG_TWO.mean, DATA_FIVE_NEG).unwrap();
        assert_eq!(v, 5.3125);

        let v = variance(MEAN_NEG_THREE.mean, DATA_FIVE_NEG).unwrap();
        assert_eq!(v, 3.75);

        let v = variance(MEAN_NEG_FIVE.mean, DATA_FIVE_NEG).unwrap();
        assert_eq!(v, 2.5);
    }

    #[test]
    fn variance_five_const() {
        let v = variance(MEAN_ZERO.mean, DATA_FIVE_CONST).unwrap();
        assert_eq!(v, 1.25);

        let v = variance(MEAN_ONE.mean, DATA_FIVE_CONST).unwrap();
        assert_eq!(v, 0.0);

        let v = variance(MEAN_TWO.mean, DATA_FIVE_CONST).unwrap();
        assert_eq!(v, 0.3125);

        let v = variance(MEAN_THREE.mean, DATA_FIVE_CONST).unwrap();
        assert_eq!(v, 1.25);

        let v = variance(MEAN_FIVE.mean, DATA_FIVE_CONST).unwrap();
        assert_eq!(v, 5.0);
    }

    #[test]
    fn std_dev_zero() {
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
    fn std_dev_one() {
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
    #[expect(clippy::approx_constant)]
    fn std_dev_two() {
        let std_dev = MEAN_ZERO.std_deviation(DATA_TWO).unwrap();
        assert_eq!(std_dev, 2.23606797749979);

        let std_dev = MEAN_ONE.std_deviation(DATA_TWO).unwrap();
        assert_eq!(std_dev, 1.0);

        let std_dev = MEAN_TWO.std_deviation(DATA_TWO).unwrap();
        assert_eq!(std_dev, 0.7071067811865476);

        let std_dev = MEAN_THREE.std_deviation(DATA_TWO).unwrap();
        assert_eq!(std_dev, 1.0);

        let std_dev = MEAN_FIVE.std_deviation(DATA_TWO).unwrap();
        assert_eq!(std_dev, 2.23606797749979);
    }

    #[test]
    fn std_dev_three() {
        let std_dev = MEAN_ZERO.std_deviation(DATA_THREE).unwrap();
        assert_eq!(std_dev, 2.6457513110645907);

        let std_dev = MEAN_ONE.std_deviation(DATA_THREE).unwrap();
        assert_eq!(std_dev, 1.5811388300841898);

        let std_dev = MEAN_TWO.std_deviation(DATA_THREE).unwrap();
        assert_eq!(std_dev, 1.1726039399558574);

        let std_dev = MEAN_THREE.std_deviation(DATA_THREE).unwrap();
        assert_eq!(std_dev, 1.0);

        let std_dev = MEAN_FIVE.std_deviation(DATA_THREE).unwrap();
        assert_eq!(std_dev, 1.5811388300841898);
    }

    #[test]
    fn std_dev_five() {
        let std_dev = MEAN_ZERO.std_deviation(DATA_FIVE).unwrap();
        assert_eq!(std_dev, 3.7080992435478315);

        let std_dev = MEAN_ONE.std_deviation(DATA_FIVE).unwrap();
        assert_eq!(std_dev, 2.7386127875258306);

        let std_dev = MEAN_TWO.std_deviation(DATA_FIVE).unwrap();
        assert_eq!(std_dev, 2.3048861143232218);

        let std_dev = MEAN_THREE.std_deviation(DATA_FIVE).unwrap();
        assert_eq!(std_dev, 1.9364916731037085);

        let std_dev = MEAN_FIVE.std_deviation(DATA_FIVE).unwrap();
        assert_eq!(std_dev, 1.5811388300841898);
    }

    #[test]
    fn std_dev_five_desc() {
        let std_dev = MEAN_ZERO.std_deviation(DATA_FIVE_DESC).unwrap();
        assert_eq!(std_dev, 3.7080992435478315);

        let std_dev = MEAN_ONE.std_deviation(DATA_FIVE_DESC).unwrap();
        assert_eq!(std_dev, 2.7386127875258306);

        let std_dev = MEAN_TWO.std_deviation(DATA_FIVE_DESC).unwrap();
        assert_eq!(std_dev, 2.3048861143232218);

        let std_dev = MEAN_THREE.std_deviation(DATA_FIVE_DESC).unwrap();
        assert_eq!(std_dev, 1.9364916731037085);

        let std_dev = MEAN_FIVE.std_deviation(DATA_FIVE_DESC).unwrap();
        assert_eq!(std_dev, 1.5811388300841898);
    }

    #[test]
    fn std_dev_five_neg() {
        let std_dev = MEAN_ZERO.std_deviation(DATA_FIVE_NEG).unwrap();
        assert_eq!(std_dev, 3.7080992435478315);

        let std_dev = MEAN_NEG_ONE.std_deviation(DATA_FIVE_NEG).unwrap();
        assert_eq!(std_dev, 2.7386127875258306);

        let std_dev = MEAN_NEG_TWO.std_deviation(DATA_FIVE_NEG).unwrap();
        assert_eq!(std_dev, 2.3048861143232218);

        let std_dev = MEAN_NEG_THREE.std_deviation(DATA_FIVE_NEG).unwrap();
        assert_eq!(std_dev, 1.9364916731037085);

        let std_dev = MEAN_NEG_FIVE.std_deviation(DATA_FIVE_NEG).unwrap();
        assert_eq!(std_dev, 1.5811388300841898);
    }

    #[test]
    fn std_dev_five_const() {
        let std_dev = MEAN_ZERO.std_deviation(DATA_FIVE_CONST).unwrap();
        assert_eq!(std_dev, 1.118033988749895);

        let std_dev = MEAN_ONE.std_deviation(DATA_FIVE_CONST);
        assert_eq!(std_dev, None);

        let std_dev = MEAN_TWO.std_deviation(DATA_FIVE_CONST).unwrap();
        assert_eq!(std_dev, 0.5590169943749475);

        let std_dev = MEAN_THREE.std_deviation(DATA_FIVE_CONST).unwrap();
        assert_eq!(std_dev, 1.118033988749895);

        let std_dev = MEAN_FIVE.std_deviation(DATA_FIVE_CONST).unwrap();
        assert_eq!(std_dev, 2.23606797749979);
    }
}
