pub trait Median {
    #[expect(
        clippy::indexing_slicing,
        clippy::integer_division,
        reason = "bounds checked by length and parity"
    )]
    fn median(mut array: Vec<Self>) -> Option<Self>
    where
        Self:
            Copy + Clone + Ord + std::ops::Add<Output = Self> + std::ops::Div<usize, Output = Self>,
    {
        if array.is_empty() {
            return None;
        }

        array.sort_unstable();

        let size = array.len();
        if size.is_multiple_of(2) {
            let left = size / 2 - 1;
            let right = size / 2;
            Some((array[left] + array[right]) / 2)
        } else {
            Some(array[size / 2])
        }
    }
}

#[cfg(test)]
mod tests {
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use super::Median;
    use crate::JsonNewMetric;

    /// A minimal `Median` implementor wrapping `OrderedFloat<f64>`,
    /// mirroring the inner value type of `JsonNewMetric`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    struct TestFloat(OrderedFloat<f64>);

    impl TestFloat {
        const fn new(value: f64) -> Self {
            Self(OrderedFloat(value))
        }
    }

    impl std::ops::Add for TestFloat {
        type Output = Self;

        fn add(self, rhs: Self) -> Self {
            Self(self.0 + rhs.0)
        }
    }

    impl std::ops::Div<usize> for TestFloat {
        type Output = Self;

        #[expect(
            clippy::cast_precision_loss,
            reason = "usize divisor to f64 is acceptable"
        )]
        fn div(self, rhs: usize) -> Self {
            Self(self.0 / rhs as f64)
        }
    }

    impl Median for TestFloat {}

    fn test_floats(values: &[f64]) -> Vec<TestFloat> {
        values.iter().copied().map(TestFloat::new).collect()
    }

    fn metric(value: f64, lower_value: Option<f64>, upper_value: Option<f64>) -> JsonNewMetric {
        JsonNewMetric {
            value: OrderedFloat(value),
            lower_value: lower_value.map(OrderedFloat),
            upper_value: upper_value.map(OrderedFloat),
        }
    }

    #[test]
    fn median_empty() {
        assert_eq!(TestFloat::median(vec![]), None);
    }

    #[test]
    fn median_single() {
        assert_eq!(
            TestFloat::median(test_floats(&[42.5])),
            Some(TestFloat::new(42.5))
        );
    }

    #[test]
    fn median_odd_length() {
        assert_eq!(
            TestFloat::median(test_floats(&[1.0, 2.0, 3.0])),
            Some(TestFloat::new(2.0))
        );
        assert_eq!(
            TestFloat::median(test_floats(&[1.0, 2.0, 3.0, 4.0, 5.0])),
            Some(TestFloat::new(3.0))
        );
    }

    #[test]
    fn median_even_length_averages_middle_two() {
        assert_eq!(
            TestFloat::median(test_floats(&[1.0, 2.0])),
            Some(TestFloat::new(1.5))
        );
        assert_eq!(
            TestFloat::median(test_floats(&[1.0, 2.0, 3.0, 4.0])),
            Some(TestFloat::new(2.5))
        );
    }

    #[test]
    fn median_duplicates() {
        assert_eq!(
            TestFloat::median(test_floats(&[5.0, 5.0, 1.0, 5.0, 5.0])),
            Some(TestFloat::new(5.0))
        );
        assert_eq!(
            TestFloat::median(test_floats(&[2.0, 4.0, 2.0, 4.0])),
            Some(TestFloat::new(3.0))
        );
        assert_eq!(
            TestFloat::median(test_floats(&[7.0, 7.0, 7.0])),
            Some(TestFloat::new(7.0))
        );
    }

    #[test]
    fn median_negative_values() {
        assert_eq!(
            TestFloat::median(test_floats(&[-1.0, -5.0, -3.0])),
            Some(TestFloat::new(-3.0))
        );
        assert_eq!(
            TestFloat::median(test_floats(&[-10.0, 10.0])),
            Some(TestFloat::new(0.0))
        );
    }

    #[test]
    fn median_unsorted_input_is_sorted() {
        assert_eq!(
            TestFloat::median(test_floats(&[9.0, 1.0, 5.0])),
            Some(TestFloat::new(5.0))
        );
        assert_eq!(
            TestFloat::median(test_floats(&[4.0, 1.0, 3.0, 2.0])),
            Some(TestFloat::new(2.5))
        );
    }

    // `OrderedFloat` implements a total order where `NaN` sorts greater than
    // all other values, so an odd-length input containing a single `NaN`
    // pushes it to the end and the median is the largest non-`NaN` value.
    #[test]
    fn median_nan_sorts_greatest() {
        assert_eq!(
            TestFloat::median(test_floats(&[1.0, f64::NAN, 2.0])),
            Some(TestFloat::new(2.0))
        );
    }

    // When `NaN` lands in the middle pair of an even-length input,
    // the average is `NaN`. `OrderedFloat` treats `NaN == NaN` as true.
    #[test]
    fn median_nan_even_length_is_nan() {
        assert_eq!(
            TestFloat::median(test_floats(&[1.0, f64::NAN])),
            Some(TestFloat::new(f64::NAN))
        );
    }

    #[test]
    fn median_all_nan() {
        assert_eq!(
            TestFloat::median(test_floats(&[f64::NAN, f64::NAN, f64::NAN])),
            Some(TestFloat::new(f64::NAN))
        );
    }

    #[test]
    fn median_metric_empty() {
        assert_eq!(JsonNewMetric::median(vec![]), None);
    }

    #[test]
    fn median_metric_odd_unsorted() {
        let metrics = vec![
            metric(3.0, Some(2.0), Some(4.0)),
            metric(1.0, Some(0.0), Some(2.0)),
            metric(2.0, Some(1.0), Some(3.0)),
        ];
        assert_eq!(
            JsonNewMetric::median(metrics),
            Some(metric(2.0, Some(1.0), Some(3.0)))
        );
    }

    #[test]
    fn median_metric_even_full_bounds() {
        let metrics = vec![
            metric(3.0, Some(2.0), Some(4.0)),
            metric(1.0, Some(0.0), Some(2.0)),
        ];
        assert_eq!(
            JsonNewMetric::median(metrics),
            Some(metric(2.0, Some(1.0), Some(3.0)))
        );
    }

    // When only one of the two middle metrics has a bound, `JsonNewMetric`'s
    // `Add` impl substitutes the other metric's central value for the missing
    // bound before averaging.
    #[test]
    fn median_metric_even_partial_bounds() {
        let metrics = vec![
            metric(10.0, None, Some(20.0)),
            metric(20.0, Some(15.0), None),
        ];
        assert_eq!(
            JsonNewMetric::median(metrics),
            Some(metric(15.0, Some(12.5), Some(20.0)))
        );
    }

    #[test]
    fn median_metric_even_no_bounds() {
        let metrics = vec![metric(10.0, None, None), metric(20.0, None, None)];
        assert_eq!(
            JsonNewMetric::median(metrics),
            Some(metric(15.0, None, None))
        );
    }
}
