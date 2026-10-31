import "@bencherdev/ui/styles.css";
import ListItem from "@bencherdev/ui/ListItem";
import Token from "@bencherdev/ui/Token";
import Truncate, { tailOf } from "@bencherdev/ui/Truncate";
import inter from "@bencherdev/ui/fonts/inter-latin-wght-normal.woff2?url";
import { For, type JSX, createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { commands, page, userEvent } from "vitest/browser";

declare module "vitest/browser" {
	interface BrowserCommands {
		ariaSnapshot: (selector: string) => Promise<string>;
	}
}

/** A 1024 character Rust path whose last segment tells it apart. */
const rustPath = (i: number, len = 1024) => {
	const tail = `::bench_fn_name_${String(i).padStart(4, "0")}`;
	let s = "my_crate";
	let k = 0;
	while (s.length < len - tail.length) {
		s += `::module_${String(k++).padStart(3, "0")}`;
	}
	return s.slice(0, len - tail.length).replace(/:+$/, "x") + tail;
};

/** A 256 byte slash branch whose leaf tells it apart. */
const longBranch = (i: number) => {
	const leaf = `/very-long-leaf-${String(i).padStart(3, "0")}`;
	let s = "release/2026/q4/team-platform";
	let k = 0;
	while (s.length < 256 - leaf.length) {
		s += `/subsystem-${k++}`;
	}
	return (s.slice(0, 256 - leaf.length) + leaf).replace(/\/\//g, "/a");
};

const PATH = rustPath(2);

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const mount = (width: number, content: () => JSX.Element) => {
	const box = document.createElement("div");
	box.style.width = `${width}px`;
	document.body.append(box);
	dispose = render(content, box);
	return box;
};

const lineHeight = (element: Element) =>
	Number.parseFloat(getComputedStyle(element).lineHeight);

/** The width the text takes drawn whole in the same font as `beside`. */
const natural = (text: string, beside: Element) => {
	const probe = document.createElement("span");
	probe.style.whiteSpace = "nowrap";
	probe.textContent = text;
	beside.after(probe);
	const width = probe.getBoundingClientRect().width;
	probe.remove();
	return width;
};

const drawnTail = (root: Element) =>
	Number.parseFloat(getComputedStyle(root, "::after").width) || 0;

// A ResizeObserver delivers its first notification before the next paint.
const frames = () =>
	new Promise((resolve) =>
		requestAnimationFrame(() => requestAnimationFrame(resolve)),
	);

test.each([
	[PATH, PATH.slice(0, -18), "bench_fn_name_0002"],
	[longBranch(0), longBranch(0).slice(0, -18), "very-long-leaf-000"],
	[
		"dependabot/cargo/serde_json-1.0.120",
		"dependabot/cargo/",
		"serde_json-1.0.120",
	],
	["my_crate::bench<a/b>", "my_crate::", "bench<a/b>"],
	["localhost", "", "localhost"],
	["release/", "release/", ""],
])("tailOf(%j) keeps the separator with the head", (text, head, tail) => {
	expect(tailOf(text)).toEqual([head, tail]);
});

// Kills a tail that shrinks, wraps, or pushes past its box.
test.each([
	[90, { tailCut: true }],
	[296, { tailCut: false }],
	[1410, { tailCut: false }],
] as const)(
	"the 1024 character path stays one line in %i px with its tail whole while it fits",
	async (width, expected) => {
		const box = mount(width, () => <Truncate text={PATH} tail />);
		const root = box.querySelector(".ui-trunc") as HTMLElement;
		await expect.poll(() => root.dataset.tail).toBe("bench_fn_name_0002");
		const rect = root.getBoundingClientRect();
		expect(rect.height).toBeLessThan(2 * lineHeight(root));
		expect(rect.right).toBeLessThanOrEqual(box.getBoundingClientRect().right);
		const whole = natural("bench_fn_name_0002", box);
		if (expected.tailCut) {
			expect(drawnTail(root)).toBeLessThan(whole - 1);
		} else {
			expect(Math.abs(drawnTail(root) - whole)).toBeLessThanOrEqual(1);
		}
		expect(root.textContent).toBe(PATH);
	},
);

// Kills a head left narrower than its ellipsis, where Chromium draws a sliver
// of the head's first glyph instead; a page title's weight has the widest one.
test.each([
	["half an ellipsis is left beside the whole tail", "400", 0.5],
	["the tail is wider than the box", "400", -2],
	["half an ellipsis is left beside the whole tail", "800", 0.5],
	["the tail is wider than the box", "800", -2],
] as const)(
	"a head draws a whole ellipsis and no sliver when %s, at weight %s",
	async (_, weight, ellipses) => {
		const beside = document.body.appendChild(document.createElement("div"));
		beside.style.fontWeight = weight;
		const inside = beside.appendChild(document.createElement("span"));
		natural("…", inside);
		await document.fonts.ready;
		const tail = natural("bench_fn_name_0002", inside);
		const ellipsis = natural("…", inside);
		beside.remove();
		const box = mount(tail + ellipses * ellipsis, () => (
			<Truncate text={PATH} tail />
		));
		box.style.fontWeight = weight;
		const root = box.querySelector(".ui-trunc") as HTMLElement;
		await expect.poll(() => root.dataset.tail).toBe("bench_fn_name_0002");
		const head = root.firstElementChild as HTMLElement;
		expect(head.getBoundingClientRect().width).toBeGreaterThanOrEqual(ellipsis);
		expect(root.getBoundingClientRect().right).toBeLessThanOrEqual(
			box.getBoundingClientRect().right + 0.01,
		);
	},
);

// Kills a value that copies, finds, or reads with a break or a doubled tail.
test.each([
	[120, false],
	[200, false],
	[200, true],
	[1410, false],
] as const)(
	"in %i px, open %s, a value copies, finds, and names exactly as written",
	async (width, open) => {
		const value = "dependabot/cargo/serde_json-1.0.120";
		const box = mount(width, () => <Truncate text={value} tail reveal />);
		const cut = width < 1410;
		if (cut) {
			const button = page.getByRole("button");
			await expect
				.element(button)
				.toHaveAttribute("data-tail", "serde_json-1.0.120");
			if (open) {
				await userEvent.click(button);
				await expect.element(button).toHaveAttribute("aria-expanded", "true");
			}
		} else {
			await frames();
			expect(box.querySelector("button")).toBeNull();
			expect(box.querySelector("[data-tail]")).toBeNull();
		}
		const root = box.querySelector(".ui-trunc") as HTMLElement;
		box.id = "truncated";
		expect(await commands.ariaSnapshot("#truncated")).toBe(
			cut
				? `- button "${value}"${open ? " [expanded]" : ""}`
				: `- text: ${value}`,
		);
		expect(root.innerText).toBe(value);
		const selection = getSelection();
		selection?.selectAllChildren(root);
		expect(selection?.toString()).toBe(value);
		selection?.removeAllRanges();
		expect(
			(window as unknown as { find: (text: string) => boolean }).find(value),
		).toBe(true);
		selection?.removeAllRanges();
	},
);

// Kills a reveal that never opens, opens without saying so, or loses characters.
test("a reveal opens the whole value in place and closes again", async () => {
	mount(296, () => <Truncate text={PATH} tail reveal />);
	const button = page.getByRole("button");
	await expect.element(button).toHaveAttribute("aria-expanded", "false");
	const element = button.element() as HTMLElement;
	const closed = element.getBoundingClientRect().height;
	await userEvent.click(button);
	await expect.element(button).toHaveAttribute("aria-expanded", "true");
	expect(element.hasAttribute("data-open")).toBe(true);
	expect(element.getBoundingClientRect().height).toBeGreaterThan(1.5 * closed);
	expect(element.textContent).toBe(PATH);
	await userEvent.click(button);
	await expect.element(button).toHaveAttribute("aria-expanded", "false");
	expect(element.hasAttribute("data-open")).toBe(false);
});

// Kills a dead Tab stop on text that fits, and a reveal that misses a resize.
test("a reveal is a button only while its box cuts the text", async () => {
	const box = mount(1410, () => <Truncate text="bench_fn_name_0002" reveal />);
	await frames();
	expect(box.querySelector("button")).toBeNull();
	box.style.width = "60px";
	await expect
		.element(page.getByRole("button", { name: "bench_fn_name_0002" }))
		.toHaveAttribute("aria-expanded", "false");
	box.style.width = "1410px";
	await expect.poll(() => box.querySelector("button")).toBeNull();
});

// Kills a Token that grows past its row or hides the end of its value.
test("a Token keeps to its row and reveals its cut value", async () => {
	const box = mount(120, () => <Token>{PATH}</Token>);
	const token = box.querySelector(".ui-token") as HTMLElement;
	expect(token.getBoundingClientRect().right).toBeLessThanOrEqual(
		box.getBoundingClientRect().right,
	);
	const button = page.getByRole("button");
	await userEvent.click(button);
	await expect.element(button).toHaveAttribute("aria-expanded", "true");
});

// Kills a long part that starves the short one or the pill beside it.
test.each([
	["localhost", false],
	["t".repeat(64), true],
] as const)(
	"a 256 byte branch beside %s in 290 px shares the line fairly",
	async (testbed, long) => {
		const box = mount(290, () => (
			<span class="ui-parts">
				<Truncate text={longBranch(0)} tail />
				<span data-fixed>·</span>
				<Truncate text={testbed} tail />
				<span data-fixed>2 alerts</span>
			</span>
		));
		const [branch, dot, bed, pill] = [
			...(box.querySelector(".ui-parts")?.children ?? []),
		] as HTMLElement[];
		const width = (element: HTMLElement | undefined) =>
			element?.getBoundingClientRect().width ?? 0;
		for (const fixed of [dot, pill]) {
			expect(fixed?.scrollWidth).toBeLessThanOrEqual(fixed?.clientWidth ?? 0);
		}
		await expect.poll(() => branch?.dataset.tail).toBe("very-long-leaf-000");
		if (long) {
			expect(Math.abs(width(branch) - width(bed))).toBeLessThanOrEqual(1);
		} else {
			expect(bed?.hasAttribute("data-cut")).toBe(false);
			expect(Math.abs(width(bed) - natural(testbed, box))).toBeLessThanOrEqual(
				1,
			);
		}
		expect(box.scrollWidth).toBeLessThanOrEqual(290);
	},
);

// Kills a fair share that lets a short part or a pill take less than it needs
// once the long part is already at nothing.
test("in 160 px the pills stay whole and the two names split what is left", async () => {
	const box = mount(160, () => (
		<span class="ui-parts">
			<Truncate text={longBranch(0)} tail />
			<span data-fixed>·</span>
			<Truncate text="localhost" tail />
			<span data-fixed>2 alerts</span>
		</span>
	));
	const [branch, dot, bed, pill] = [
		...(box.querySelector(".ui-parts")?.children ?? []),
	] as HTMLElement[];
	for (const fixed of [dot, pill]) {
		expect(fixed?.scrollWidth).toBeLessThanOrEqual(fixed?.clientWidth ?? 0);
	}
	const width = (element: HTMLElement | undefined) =>
		element?.getBoundingClientRect().width ?? 0;
	expect(width(bed)).toBeLessThan(natural("localhost", box));
	expect(Math.abs(width(branch) - width(bed))).toBeLessThanOrEqual(1);
	expect(box.scrollWidth).toBeLessThanOrEqual(160);
});

// Kills parts that spread across a row with room to spare.
test("parts that fit sit side by side at their own widths", () => {
	const box = mount(600, () => (
		<span class="ui-parts">
			<Truncate text="main" tail />
			<span data-fixed>·</span>
			<Truncate text="localhost" tail />
		</span>
	));
	const [main, , bed] = [...(box.querySelector(".ui-parts")?.children ?? [])];
	const width = (element: Element | undefined) =>
		element?.getBoundingClientRect().width ?? 0;
	expect(Math.abs(width(main) - natural("main", box))).toBeLessThanOrEqual(1);
	expect(Math.abs(width(bed) - natural("localhost", box))).toBeLessThanOrEqual(
		1,
	);
});

/** Every ResizeObserver loop error the page reports while `run` runs. */
const loopErrors = async (run: () => Promise<void>) => {
	const errors: string[] = [];
	const listen = (event: ErrorEvent) => {
		if (event.message.includes("ResizeObserver")) {
			errors.push(event.message);
			event.preventDefault();
		}
	};
	window.addEventListener("error", listen);
	try {
		await run();
		await frames();
	} finally {
		window.removeEventListener("error", listen);
	}
	return errors;
};

/** Rows whose name shares its line with a sibling that also shrinks; every other one reveals. */
const sharedRows = (count: number) => (
	<For each={Array.from({ length: count }, (_, i) => i)}>
		{(i) => (
			<ListItem>
				<Truncate text={longBranch(i)} tail reveal={i % 2 === 1} />
				<span>12 runs · 3 days ago</span>
			</ListItem>
		)}
	</For>
);

// Kills a drawn tail that changes the space its row gives it, which starts a
// second round of layout inside the observer's own notice.
test.each([320, 768, 1440])(
	"a row whose sibling also shrinks mounts at %i px without a layout loop",
	async (width) => {
		await page.viewport(width, 900);
		let box: HTMLElement | undefined;
		const errors = await loopErrors(async () => {
			box = mount(width, () => sharedRows(1));
			await frames();
		});
		expect(errors).toEqual([]);
		expect(box?.querySelector("[data-cut]")).not.toBeNull();
	},
);

test("500 rows whose siblings also shrink resize without a layout loop", async () => {
	const box = mount(1400, () => sharedRows(500));
	const errors = await loopErrors(async () => {
		await frames();
		for (const width of [1400, 300, 1400, 600, 320]) {
			box.style.width = `${width}px`;
			await frames();
		}
	});
	expect(errors).toEqual([]);
});

// Kills a cut that holds on in the band where the whole value fits again.
test("a box cuts by its width alone, whatever it was before", async () => {
	const value = "dependabot/cargo/serde_json-1.0.120";
	const beside = document.body.appendChild(document.createElement("div"));
	const room = Math.ceil(natural(value, beside)) + 4;
	beside.remove();
	const [width, setWidth] = createSignal(160);
	const box = mount(1410, () => (
		<>
			<div class="grown" style={{ width: `${width()}px` }}>
				<Truncate text={value} tail />
			</div>
			<div class="fresh" style={{ width: `${room}px` }}>
				<Truncate text={value} tail />
			</div>
		</>
	));
	const cutIn = (name: string) =>
		box.querySelector(`.${name} .ui-trunc`)?.hasAttribute("data-cut");
	await expect.poll(() => cutIn("grown")).toBe(true);
	setWidth(room);
	await frames();
	expect(cutIn("fresh")).toBe(false);
	expect(cutIn("grown")).toBe(cutIn("fresh"));
	setWidth(room - 8);
	await expect.poll(() => cutIn("grown")).toBe(true);
});

// Kills a stale cut kept by a value that no longer has a tail to draw.
test("a new value without a separator is measured afresh", async () => {
	const [value, setValue] = createSignal(PATH);
	const box = mount(296, () => <Truncate text={value()} tail />);
	await expect.poll(() => box.querySelector("[data-cut]")).not.toBeNull();
	setValue("localhost");
	await expect.poll(() => box.querySelector("[data-cut]")).toBeNull();
	expect(box.querySelector(".ui-trunc")?.getAttribute("style") ?? "").toBe("");
});

// Kills a reveal that drops the focus to the page when it closes on a box
// that no longer cuts it.
test("a reveal closed after it stops cutting keeps the focus", async () => {
	const box = mount(200, () => <Truncate text={PATH.slice(-60)} tail reveal />);
	const button = page.getByRole("button");
	await userEvent.click(button);
	await expect.element(button).toHaveAttribute("aria-expanded", "true");
	box.style.width = "1400px";
	await frames();
	await userEvent.click(button);
	await expect.element(button).toHaveAttribute("aria-expanded", "false");
	await frames();
	const root = box.querySelector(".ui-trunc");
	expect(root?.hasAttribute("data-cut")).toBe(false);
	expect(document.activeElement).toBe(root);
});

/** The width a text node takes drawn whole on one line. */
const textWidth = (node: Node) => {
	const range = document.createRange();
	range.selectNodeContents(node);
	return range.getBoundingClientRect().width;
};

// Kills a cut that misses a web font landing: the box stays put while the
// text inside it grows or shrinks.
test("a web font that lands after mount re-cuts every value by its new width", async () => {
	const faces = { wide: 1.6, narrow: 0.6 } as const;
	const values = Array.from(
		{ length: 20 },
		(_, i) => `dependabot/cargo/serde_json-1.0.${100 + i}`,
	);
	const rows: { family: string; box: number; value: string }[] = [];
	const beside = document.body.appendChild(document.createElement("div"));
	for (const family of Object.keys(faces)) {
		beside.style.fontFamily = `trunc-${family}, sans-serif`;
		for (const value of values) {
			const fallback = natural(value, beside);
			// Each value in a box it just fits or just misses, and in one it
			// misses by far, so the font flips the first and resizes the second.
			const near = family === "wide" ? fallback + 2 : fallback - 2;
			rows.push({ family, box: near, value }, { family, box: 60, value });
		}
	}
	beside.remove();
	const box = mount(1410, () => (
		<For each={rows}>
			{(row) => (
				<div
					style={{
						width: `${row.box}px`,
						"font-family": `trunc-${row.family}, sans-serif`,
					}}
				>
					<Truncate text={row.value} tail />
				</div>
			)}
		</For>
	));
	await frames();
	const style = document.createElement("style");
	style.textContent = Object.entries(faces)
		.map(
			([family, scale]) =>
				`@font-face { font-family: trunc-${family}; src: url(${inter}); size-adjust: ${scale * 100}%; }`,
		)
		.join("\n");
	document.head.append(style);
	try {
		await Promise.all(
			Object.keys(faces).map((family) =>
				document.fonts.load(`16px trunc-${family}`),
			),
		);
		await document.fonts.ready;
		await frames();
		await frames();
		const found = [...box.querySelectorAll<HTMLElement>(".ui-trunc")].map(
			(root) => {
				const text = root.firstChild as Node;
				const whole = textWidth(text);
				const space = (root.parentElement as HTMLElement).clientWidth;
				const held = Number.parseFloat(
					root.style.getPropertyValue("--ui-trunc-whole"),
				);
				return {
					cut: root.hasAttribute("data-cut"),
					wider: whole > space + 0.01,
					held: root.hasAttribute("data-cut")
						? Math.abs(held - whole) <= 0.01
						: true,
				};
			},
		);
		expect(found.filter((row) => row.cut !== row.wider)).toEqual([]);
		expect(found.filter((row) => !row.held)).toEqual([]);
		expect(found.some((row) => row.cut)).toBe(true);
		expect(found.some((row) => !row.cut)).toBe(true);
	} finally {
		style.remove();
	}
});
