import { readFileSync } from "node:fs";
import { describe, expect, test } from "vitest";

const THEME_CSS = readFileSync(
	new URL("../../../../../packages/ui/src/styles/theme.css", import.meta.url),
	"utf8",
);

const declarations = (selector: string): Map<string, string> => {
	const start = THEME_CSS.indexOf(`${selector} {`);
	if (start === -1) {
		throw new Error(`theme.css has no \`${selector}\` block`);
	}
	const body = THEME_CSS.slice(start, THEME_CSS.indexOf("}", start));
	const tokens = new Map<string, string>();
	for (const [, name, value] of body.matchAll(/(--[\w-]+):\s*([^;]+);/g)) {
		if (name !== undefined && value !== undefined) {
			tokens.set(name, value.trim());
		}
	}
	return tokens;
};

const DARK = declarations(":root");
const LIGHT_OVERRIDES = declarations(':root[data-theme="light"]');
// The light block only redefines; anything it leaves out cascades from dark.
const LIGHT = new Map([...DARK, ...LIGHT_OVERRIDES]);
const THEMES = [
	["dark", DARK],
	["light", LIGHT],
] as const;

type Rgb = readonly [number, number, number];
type Rgba = readonly [number, number, number, number];

const parseColor = (value: string): Rgba => {
	const hex = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(value);
	if (hex) {
		const [, r = "", g = "", b = ""] = hex;
		return [
			Number.parseInt(r, 16),
			Number.parseInt(g, 16),
			Number.parseInt(b, 16),
			1,
		];
	}
	const rgba =
		/^rgba?\(\s*(\d+),\s*(\d+),\s*(\d+)(?:,\s*(\d*\.?\d+))?\s*\)$/.exec(value);
	if (rgba) {
		const [, r = "", g = "", b = "", a = "1"] = rgba;
		return [Number(r), Number(g), Number(b), Number(a)];
	}
	throw new Error(`Cannot parse the color \`${value}\``);
};

const token = (theme: Map<string, string>, name: string): Rgba => {
	const value = theme.get(name);
	if (value === undefined) {
		throw new Error(`The theme has no \`${name}\``);
	}
	return parseColor(value);
};

// Paints each layer over the one before it; the first layer is the opaque ground.
const paint = (theme: Map<string, string>, layers: readonly string[]): Rgb => {
	let rgb: Rgb = [0, 0, 0];
	for (const layer of layers) {
		const [r, g, b, a] = token(theme, layer);
		rgb = [
			r * a + rgb[0] * (1 - a),
			g * a + rgb[1] * (1 - a),
			b * a + rgb[2] * (1 - a),
		];
	}
	return rgb;
};

const linear = (channel: number): number => {
	const c = channel / 255;
	return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
};

const luminance = ([r, g, b]: Rgb): number =>
	0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);

const contrast = (a: Rgb, b: Rgb): number => {
	const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
	return ((hi ?? 0) + 0.05) / ((lo ?? 0) + 0.05);
};

const BODY = "--color-background-body";
const SURFACE = "--color-background-surface";
const CARD = "--color-background-card";
const HOVER = "--color-overlay-hover";

// Every ground the console paints text and controls on, as the layers that make it.
const GROUNDS: Record<string, readonly string[]> = {
	body: [BODY],
	surface: [SURFACE],
	card: [BODY, CARD],
	"muted background": [BODY, "--color-background-muted"],
	"hover on the body": [BODY, HOVER],
	"hover on a card": [BODY, CARD, HOVER],
	"hover on a surface": [SURFACE, HOVER],
	plot: ["--color-background-plot"],
	"alerting row": [BODY, "--color-error-muted"],
	"warning tint": [BODY, "--color-warning-muted"],
	"success tint": [BODY, "--color-success-muted"],
	"selected segment": [BODY, "--color-accent-muted"],
	code: [BODY, "--color-syntax-background"],
};

// A tag carries its own text, never a link or a status.
const TAG_GROUNDS: Record<string, readonly string[]> = {
	tag: [BODY, "--color-background-tag"],
	"parameter tag": [BODY, "--color-background-blue"],
};

const TAG_TEXT = [
	"--color-text-primary",
	"--color-text-secondary",
	"--color-text-muted",
	"--color-text-blue",
];

const TEXT = [
	"--color-text-primary",
	"--color-text-secondary",
	"--color-text-muted",
	"--color-text-eyebrow",
	"--color-text-faint",
	"--color-text-accent",
	"--color-text-success",
	"--color-text-warning",
	"--color-text-error",
	"--color-text-blue",
	"--color-worse",
	"--color-better",
];

const SERIES = Array.from(
	{ length: 8 },
	(_, i) => `--color-data-categorical-${i + 1}`,
);

// The grounds where `token`, painted over each one, falls under `minimum`.
const shortfalls = (
	theme: Map<string, string>,
	token: string,
	minimum: number,
	grounds = GROUNDS,
): string[] =>
	Object.entries(grounds).flatMap(([name, layers]) => {
		const ratio = contrast(
			paint(theme, [...layers, token]),
			paint(theme, layers),
		);
		return ratio < minimum ? [`${name}: ${ratio.toFixed(2)}:1`] : [];
	});

describe.each(THEMES)("the %s theme", (_, theme) => {
	test.each(TEXT)("%s is at least 4.5:1 on every ground", (text) => {
		expect(shortfalls(theme, text, 4.5)).toEqual([]);
	});

	test.each(TAG_TEXT)("%s is at least 4.5:1 on every tag", (text) => {
		expect(shortfalls(theme, text, 4.5, TAG_GROUNDS)).toEqual([]);
	});

	test.each([
		["--color-on-accent", "--color-accent"],
		["--color-on-accent", "--color-accent-hover"],
		["--color-on-marker", "--color-marker"],
	])("%s is at least 4.5:1 on %s", (text, fill) => {
		expect(
			contrast(paint(theme, [fill, text]), paint(theme, [fill])),
		).toBeGreaterThanOrEqual(4.5);
	});

	test.each([
		"--color-stroke-accent",
		"--color-focus",
		"--color-border-control",
	])("%s is at least 3:1 on every ground", (stroke) => {
		expect(shortfalls(theme, stroke, 3)).toEqual([]);
	});

	test.each(SERIES)("%s is at least 3:1 on the plot", (series) => {
		expect(
			contrast(
				paint(theme, ["--color-background-plot", series]),
				paint(theme, ["--color-background-plot"]),
			),
		).toBeGreaterThanOrEqual(3);
	});
});

test("the light theme redefines every token the dark theme sets", () => {
	expect([...LIGHT_OVERRIDES.keys()].sort()).toEqual([...DARK.keys()].sort());
});

// Machado, Oliveira, and Fernandes (2009) at full severity, applied to linear sRGB.
const DEFICIENCIES = {
	protanopia: [
		[0.152286, 1.052583, -0.204868],
		[0.114503, 0.786281, 0.099216],
		[-0.003882, -0.048116, 1.051998],
	],
	deuteranopia: [
		[0.367322, 0.860646, -0.227968],
		[0.280085, 0.672501, 0.047413],
		[-0.01182, 0.04294, 0.968881],
	],
	tritanopia: [
		[1.255528, -0.076749, -0.178779],
		[-0.078411, 0.930809, 0.147602],
		[0.004733, 0.691367, 0.3039],
	],
} as const;

type Matrix = readonly (readonly [number, number, number])[];

const simulate = (matrix: Matrix, [r, g, b]: Rgb): Rgb => {
	const [x, y, z] = matrix.map(([m0, m1, m2]) =>
		Math.min(1, Math.max(0, m0 * r + m1 * g + m2 * b)),
	);
	return [x ?? 0, y ?? 0, z ?? 0];
};

// Ottosson's OKLab, from linear sRGB.
const toOklab = ([r, g, b]: Rgb): Rgb => {
	const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
	const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
	const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
	return [
		0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
		1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
		0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
	];
};

// OKLab, not CIEDE2000, which is fit to small differences and turns irregular
// at the distances series need to keep. 0.08 is about four OKLab
// just-noticeable differences.
const MINIMUM_SERIES_DISTANCE = 0.08;

describe.each(THEMES)("the %s series", (_, theme) => {
	const linearSeries = SERIES.map((series) => {
		const [r, g, b] = token(theme, series);
		return [linear(r), linear(g), linear(b)] as const;
	});

	const tooClose = (seen: Rgb[], pairs: [number, number][]): string[] =>
		pairs.flatMap(([i, j]) => {
			const [a, b] = [seen[i], seen[j]];
			const distance =
				a && b ? Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]) : 0;
			return distance < MINIMUM_SERIES_DISTANCE
				? [`slots ${i + 1} and ${j + 1}: ${distance.toFixed(3)}`]
				: [];
		});

	test.each(Object.entries(DEFICIENCIES))(
		"neighbors stay apart under %s",
		(_, matrix) => {
			const seen = linearSeries.map((rgb) => toOklab(simulate(matrix, rgb)));
			const neighbors = seen.map((_, i): [number, number] => [
				i,
				(i + 1) % seen.length,
			]);
			expect(tooClose(seen, neighbors)).toEqual([]);
		},
	);

	test("every two slots stay apart in typical vision", () => {
		const seen = linearSeries.map(toOklab);
		const pairs = seen.flatMap((_, i) =>
			seen.slice(i + 1).map((_, k): [number, number] => [i, i + 1 + k]),
		);
		expect(tooClose(seen, pairs)).toEqual([]);
	});
});
