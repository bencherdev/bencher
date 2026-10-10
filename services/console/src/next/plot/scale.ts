import type { PlotScale } from "./types";

export type YScale =
	| { readonly kind: "linear" }
	| { readonly kind: "pow"; readonly exponent: number }
	| { readonly kind: "log" };

const LINEAR: YScale = { kind: "linear" };
const LOG: YScale = { kind: "log" };

export const yScale = (mode: PlotScale, min: number, max: number): YScale => {
	if (mode === "linear") {
		return LINEAR;
	}
	if (mode === "log" && min > 0) {
		return LOG;
	}
	return autoScale(min, max);
};

// Linear up to a tenfold spread, else a power scale that flattens as the spread grows.
const autoScale = (min: number, max: number): YScale => {
	const spread = max / min;
	if (!(spread > 10)) {
		return LINEAR;
	}
	return { kind: "pow", exponent: Math.max(1 / Math.log10(spread), 1 / 3) };
};

export interface Transform {
	fwd: (value: number) => number;
	bwd: (position: number) => number;
}

const IDENTITY: Transform = { fwd: (value) => value, bwd: (value) => value };
const LOG10: Transform = {
	fwd: Math.log10,
	bwd: (position) => 10 ** position,
};

export const transform = (scale: YScale): Transform => {
	switch (scale.kind) {
		case "linear":
			return IDENTITY;
		case "log":
			return LOG10;
		case "pow": {
			const { exponent } = scale;
			return {
				fwd: (value) => Math.sign(value) * Math.abs(value) ** exponent,
				bwd: (position) =>
					Math.sign(position) * Math.abs(position) ** (1 / exponent),
			};
		}
	}
};

const PAD_BELOW = 0.16;
const PAD_ABOVE = 0.18;

/** The y range for values from `min` to `max`, padded in the scale's own space. */
export const yRange = (
	scale: YScale,
	min: number,
	max: number,
	below = PAD_BELOW,
	above = PAD_ABOVE,
): [number, number] => {
	const { fwd, bwd } = transform(scale);
	const low = fwd(min);
	const high = fwd(max);
	const span = high - low || Math.abs(high) * 0.1 || 1;
	let lo = bwd(low - span * below);
	const hi = bwd(high + span * above);
	if (scale.kind !== "log" && min >= 0 && lo < 0) {
		lo = 0;
	}
	return [lo, hi];
};

export interface Ticks {
	ticks: number[];
	/** The distance between linear ticks, or 0 for ticks with no common step. */
	step: number;
}

export const yTicks = (
	scale: YScale,
	lo: number,
	hi: number,
	count: number,
): Ticks => {
	if (scale.kind === "log") {
		const powers = powersOfTen(lo, hi, count);
		if (powers.length >= 3) {
			return { ticks: powers, step: 0 };
		}
	}
	if (scale.kind !== "linear") {
		const ticks = nearestNice(scale, lo, hi, count);
		if (ticks.length >= 3) {
			return { ticks, step: 0 };
		}
	}
	return linearTicks(lo, hi, count);
};

const linearTicks = (lo: number, hi: number, count: number): Ticks => {
	const raw = (hi - lo) / Math.max(1, count - 1);
	const step = niceStep(raw > 0 ? raw : Math.abs(lo) * 0.1 || 1);
	const ticks: number[] = [];
	for (
		let tick = Math.ceil(lo / step) * step;
		tick <= hi + step * 1e-6;
		tick += step
	) {
		ticks.push(Number(tick.toPrecision(12)));
	}
	return { ticks, step };
};

const niceStep = (raw: number): number => {
	const power = 10 ** Math.floor(Math.log10(raw));
	const mantissa = raw / power;
	const nice =
		mantissa < 1.5
			? 1
			: mantissa < 2.25
				? 2
				: mantissa < 3.5
					? 2.5
					: mantissa < 7.5
						? 5
						: 10;
	return nice * power;
};

const powersOfTen = (lo: number, hi: number, count: number): number[] => {
	const powers: number[] = [];
	if (!(lo > 0)) {
		return powers;
	}
	for (
		let exponent = Math.ceil(Math.log10(lo));
		10 ** exponent <= hi;
		exponent++
	) {
		powers.push(Number((10 ** exponent).toPrecision(12)));
	}
	const stride = Math.ceil(powers.length / count);
	return powers.filter((_, index) => index % stride === 0);
};

// 1, 2, and 5 times a power of ten, each the closest to one of `count` even positions in the scale's own space.
const nearestNice = (
	scale: YScale,
	lo: number,
	hi: number,
	count: number,
): number[] => {
	const { fwd } = transform(scale);
	const low = fwd(lo);
	const high = fwd(hi);
	const candidates = niceCandidates(lo, hi).filter((value) => {
		const position = fwd(value);
		return position >= low && position <= high;
	});
	const ticks = new Set<number>();
	for (let index = 0; index < count; index++) {
		const target = low + ((high - low) * (index + 0.5)) / count;
		let best: number | undefined;
		for (const candidate of candidates) {
			if (
				best === undefined ||
				Math.abs(fwd(candidate) - target) < Math.abs(fwd(best) - target)
			) {
				best = candidate;
			}
		}
		if (best !== undefined) {
			ticks.add(best);
		}
	}
	return [...ticks].sort((a, b) => a - b);
};

const niceCandidates = (lo: number, hi: number): number[] => {
	const candidates = lo <= 0 && hi >= 0 ? [0] : [];
	const top = Math.ceil(Math.log10(Math.max(Math.abs(lo), Math.abs(hi))));
	const smallest = Math.min(...[lo, hi].map(Math.abs).filter((v) => v > 0));
	const bottom = Math.floor(Math.log10(smallest)) - 1;
	for (let exponent = bottom; exponent <= top; exponent++) {
		for (const mantissa of [1, 2, 5]) {
			const value = Number((mantissa * 10 ** exponent).toPrecision(12));
			candidates.push(value);
			if (lo < 0) {
				candidates.push(-value);
			}
		}
	}
	return candidates;
};
