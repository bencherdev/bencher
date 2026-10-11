import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../reports/reports.css";
import "./alerts.css";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import type { Api } from "../api";
import { ProjectContext } from "../project";
import Controls from "./Controls";
import { type AlertsSearch, DEFAULT_SEARCH } from "./search";

const DAY = 24 * 60 * 60 * 1_000;
const NOW = Date.parse("2026-09-14T12:00:00Z");

let dispose: (() => void) | undefined;

const api: Api = {
	get: () => new Promise(() => {}),
	send: () => new Promise(() => {}),
};

const mount = (search: AlertsSearch = DEFAULT_SEARCH) => {
	const onSearch = vi.fn<(search: AlertsSearch) => void>();
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<QueryClientProvider client={new QueryClient()}>
				<ProjectContext.Provider value={{ api, slug: () => "hashbrown" }}>
					<Controls search={search} names={{}} now={NOW} onSearch={onSearch} />
				</ProjectContext.Provider>
			</QueryClientProvider>
		),
		root,
	);
	return onSearch;
};

const windowRadio = (name: string) =>
	page.getByRole("radiogroup", { name: "Window" }).getByRole("radio", { name });

beforeEach(async () => {
	await page.viewport(1280, 720);
});

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

// Kills a window position that sends another window, a custom range that
// does not start four weeks back to today, and a status that drops the window.
test("each window position and status sends the search it names", async () => {
	const onSearch = mount();
	await windowRadio("All").click();
	expect(onSearch).toHaveBeenLastCalledWith({
		...DEFAULT_SEARCH,
		window: { kind: "all" },
	});
	await windowRadio("1w").click();
	expect(onSearch).toHaveBeenLastCalledWith({
		...DEFAULT_SEARCH,
		window: { kind: "rolling", days: 7 },
	});
	await windowRadio("Custom").click();
	const custom = onSearch.mock.lastCall?.[0].window;
	expect(custom?.kind).toBe("custom");
	if (custom?.kind === "custom") {
		expect(Math.round((custom.end - custom.start) / DAY)).toBe(28);
		expect(custom.end).toBeGreaterThanOrEqual(NOW);
		expect(custom.end - NOW).toBeLessThan(DAY);
	}
	await page
		.getByRole("radiogroup", { name: "Status" })
		.getByRole("radio", { name: "Dismissed" })
		.click();
	expect(onSearch).toHaveBeenLastCalledWith({
		...DEFAULT_SEARCH,
		status: "dismissed",
	});
});

// Kills a custom window drawn without its range, and a range edit that sends nothing.
test("a custom window shows its range, and editing it sends the new range", async () => {
	const start = Date.parse("2026-09-01T00:00:00");
	const end = Date.parse("2026-09-10T23:59:59.999");
	const onSearch = mount({
		...DEFAULT_SEARCH,
		window: { kind: "custom", start, end },
	});
	await expect.element(windowRadio("Custom")).toBeChecked();
	const from = page.getByLabelText("From", { exact: true });
	await expect.element(from).toHaveValue("2026-09-01");
	await from.fill("2026-09-05");
	(from.element() as HTMLInputElement).dispatchEvent(
		new Event("change", { bubbles: true }),
	);
	expect(onSearch).toHaveBeenLastCalledWith({
		...DEFAULT_SEARCH,
		window: {
			kind: "custom",
			start: Date.parse("2026-09-05T00:00:00"),
			end,
		},
	});
});
