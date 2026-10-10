import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "./report.css";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import Controls from "./Controls";
import type { ReportView } from "./view";

const VIEW: ReportView = {
	group: "benchmark",
	sort: "name",
	window: "4w",
	search: "",
	expanded: [],
};

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

/** The controls over a view the page keeps, as its URL would, recording each write and whether it replaced the entry. */
const mount = (initial: Partial<ReportView> = {}) => {
	const writes: { view: ReportView; replace: boolean }[] = [];
	const [view, setView] = createSignal<ReportView>({ ...VIEW, ...initial });
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<Controls
				view={view()}
				narrow={false}
				onView={(next, replace = false) => {
					writes.push({ view: next, replace });
					setView(next);
				}}
			/>
		),
		root,
	);
	return { writes, setView };
};

const filter = () =>
	page.getByRole("searchbox", {
		name: "Filter lines by benchmark, parameter, measure, or metric",
	});
const pause = () => new Promise((resolve) => setTimeout(resolve, 400));

// Kills a filter whose trimmed write comes back into the box and eats a typed space, and one that pushes history.
test("the filter keeps a space typed before a pause", async () => {
	const { writes } = mount();
	await filter().click();
	await userEvent.keyboard("blake3 ");
	await pause();
	await userEvent.keyboard("x");
	await pause();
	await expect.element(filter()).toHaveValue("blake3 x");
	expect(writes.map(({ view, replace }) => [view.search, replace])).toEqual([
		["blake3", true],
		["blake3 x", true],
	]);
});

// Kills a box that ignores a filter changed elsewhere, as by Back.
test("a filter changed elsewhere comes back into the box", async () => {
	const { setView } = mount({ search: "avx2" });
	await expect.element(filter()).toHaveValue("avx2");
	setView({ ...VIEW, search: "sse4" });
	await expect.element(filter()).toHaveValue("sse4");
});

// Kills a custom window that forgets the length it came from, and a days field that sets nothing.
test("a custom window starts from the window's length, and the days field sets it", async () => {
	const { writes } = mount();
	await page.getByRole("radio", { name: "Custom" }).click();
	expect(writes.at(-1)?.view.window).toBe(28);

	const days = page.getByRole("spinbutton", {
		name: "Window in days, ending at this report",
	});
	await expect.element(days).toHaveValue(28);
	await days.fill("10");
	await userEvent.keyboard("{Tab}");
	await expect.poll(() => writes.at(-1)?.view.window).toBe(10);
});
