// The percent difference between a value and its baseline.
//
// Only the baseline has to be non-zero.
// A value of zero is a `-100.00%` difference from a positive baseline,
// not a `0.00%` difference.
// There is no percent difference from a zero baseline (`±∞`)
// nor from a non-finite value, so fall back to zero.
export const percentDifference = (value: number, baseline: number): number =>
	Number.isFinite(value) && Number.isFinite(baseline) && baseline !== 0
		? ((value - baseline) / baseline) * 100
		: 0.0;

// The numerator as a percent of the denominator.
//
// Only the denominator has to be non-zero.
// A numerator of zero is `0.00%` of a positive denominator.
// There is no percent of a zero denominator (`±∞`)
// nor of a non-finite numerator, so fall back to zero.
export const percentOf = (numerator: number, denominator: number): number =>
	Number.isFinite(numerator) &&
	Number.isFinite(denominator) &&
	denominator !== 0
		? (numerator / denominator) * 100
		: 0.0;
