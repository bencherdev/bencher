import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, test } from "vitest";

const REPO = fileURLToPath(new URL("../../../../../", import.meta.url));
const THEME_FILE = "packages/ui/src/styles/theme.css";
const ROOTS = [
	"packages/ui/src",
	"services/console/src/next",
	"services/console/src/pages/next",
];

test("no color is written outside the theme", () => {
	expect(
		sources().flatMap(({ path, source }) =>
			findColorLiterals(source).map(
				({ line, text }) => `${path}:${line}: ${text}`,
			),
		),
	).toEqual([]);
});

test("every color token in use is one the theme defines", () => {
	const defined = new Set(
		readFileSync(join(REPO, THEME_FILE), "utf8").match(/--color-[\w-]+(?=:)/g),
	);
	const undefinedTokens = sources().flatMap(({ path, source }) =>
		[...source.matchAll(/var\((--color-[\w-]+)/g)].flatMap(([, name]) =>
			name === undefined || defined.has(name) ? [] : [`${path}: ${name}`],
		),
	);
	expect(undefinedTokens).toEqual([]);
});

describe("findColorLiterals", () => {
	test.each([
		["a hex color", "a { color: #fff; }", "#fff"],
		["a hex color with alpha", '<path fill="#ed6704cc" />', "#ed6704cc"],
		["rgb()", "box-shadow: 0 0 0 1px rgb(0 0 0 / 50%);", "rgb("],
		["rgba()", "background: rgba(0, 0, 0, 0.5);", "rgba("],
		["hsl()", "color: hsl(20 90% 50%);", "hsl("],
		["oklch()", "color: oklch(70% 0.1 50);", "oklch("],
		["color()", "color: color(srgb 1 0 0);", "color("],
		["a named color in a declaration", "border: 1px solid red;", "red"],
		[
			"a named color in a style object",
			'{ backgroundColor: "White" }',
			"White",
		],
		["a named color in an svg attribute", '<path stroke="black" />', "black"],
		[
			"a named color under a quoted property",
			'style={{ "background-color": "red" }}',
			"red",
		],
		["a named color in a custom property", "--tone: red;", "red"],
		[
			"a named color in a background image",
			"background-image: linear-gradient(white, black);",
			"white",
		],
		[
			"a named color in a mask",
			"mask: linear-gradient(black, transparent);",
			"black",
		],
		[
			"a named color in a mask image",
			"mask-image: radial-gradient(black, transparent);",
			"black",
		],
		[
			"a named color in a filter",
			"filter: drop-shadow(0 0 2px black);",
			"black",
		],
	])("finds %s", (_, source, text) => {
		expect(findColorLiterals(source)).toEqual([{ line: 1, text }]);
	});

	test.each([
		["transparent", "background: transparent;"],
		["currentColor", '<svg stroke="currentColor" fill="none">'],
		["inherit", "color: inherit;"],
		["a token named after a color", "color: var(--color-text-red);"],
		["a color word that is not a color property", 'tone: "red"'],
		["a numeric character reference", "&#123;"],
		["an id selector", "a[href='#main'] {}"],
	])("allows %s", (_, source) => {
		expect(findColorLiterals(source)).toEqual([]);
	});

	test("reports the line of each literal", () => {
		expect(
			findColorLiterals("a {\n\tcolor: #fff;\n}\nb { fill: red; }"),
		).toEqual([
			{ line: 2, text: "#fff" },
			{ line: 4, text: "red" },
		]);
	});
});

const SOURCE = /\.(?:astro|css|html|jsx?|mjs|sass|scss|svg|tsx?)$/;
const TEST = /\.test\.[jt]sx?$/;

// Every source file under the roots but the theme itself and the tests, whose fixtures need literals.
const sources = (): { path: string; source: string }[] =>
	ROOTS.flatMap((root) => walk(root));

const walk = (dir: string): { path: string; source: string }[] => {
	if (!existsSync(join(REPO, dir))) {
		return [];
	}
	return readdirSync(join(REPO, dir), { withFileTypes: true }).flatMap(
		(entry) => {
			const path = `${dir}/${entry.name}`;
			if (entry.isDirectory()) {
				return entry.name === "node_modules" ? [] : walk(path);
			}
			if (path === THEME_FILE || !SOURCE.test(path) || TEST.test(path)) {
				return [];
			}
			return [{ path, source: readFileSync(join(REPO, path), "utf8") }];
		},
	);
};

const HEX = /(?<![\w&#.])#(?:[0-9a-f]{8}|[0-9a-f]{6}|[0-9a-f]{3,4})(?![\w-])/gi;
const COLOR_FUNCTION = /\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\(/gi;
// A property or attribute that can take a color, quoted or not, then its value up to the end of the declaration.
const COLOR_PROPERTY =
	/(?<![\w-])(?:--[\w-]+|color|background(?:-?color)?|(?:background|mask)-?image|mask|filter|border(?:-?(?:top|right|bottom|left|block|inline)(?:-?(?:start|end))?)?(?:-?color)?|outline(?:-?color)?|fill|stroke|(?:box|text)-?shadow|caret-?color|accent-?color|column-?rule(?:-?color)?|text-?decoration(?:-?color)?|stop-?color|flood-?color|lighting-?color|scrollbar-?color)["']?\s*[:=]\s*([^;{}\n]+)/gi;
const CUSTOM_PROPERTY = /--[\w-]+/g;
const NAMED_COLOR = new RegExp(
	`(?<![\\w-])(?:${"aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen".replaceAll(" ", "|")})(?![\\w-])`,
	"i",
);

const findColorLiterals = (
	source: string,
): { line: number; text: string }[] => {
	const found: { index: number; text: string }[] = [];
	for (const pattern of [HEX, COLOR_FUNCTION]) {
		for (const match of source.matchAll(pattern)) {
			found.push({ index: match.index, text: match[0] });
		}
	}
	for (const match of source.matchAll(COLOR_PROPERTY)) {
		const value = match[1] ?? "";
		const named = NAMED_COLOR.exec(value.replace(CUSTOM_PROPERTY, ""));
		if (named) {
			found.push({
				index: match.index + match[0].indexOf(named[0]),
				text: named[0],
			});
		}
	}
	return found
		.sort((a, b) => a.index - b.index)
		.map(({ index, text }) => ({
			line: source.slice(0, index).split("\n").length,
			text,
		}));
};
