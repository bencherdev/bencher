import { THEME_ATTRIBUTE } from "@bencherdev/ui/ThemeToggle";
import { PostMessageTransport } from "@modelcontextprotocol/ext-apps";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import { boot } from "./main";
import {
	CONSOLE,
	INPUT,
	URL_,
	fakeHost,
	perf,
	success,
	toolError,
	uuid,
} from "./testing";

/** The fake host, answering the page in this window. */
const host = (options: Parameters<typeof fakeHost>[0]) => {
	const shown = fakeHost(options, (message) =>
		window.postMessage(message, "*"),
	);
	// The page posts to the host, whose answers arrive as messages from this window.
	const transport = new PostMessageTransport(
		{ postMessage: shown.answer } as unknown as Window,
		window,
	);
	return { ...shown, transport };
};

let close: (() => Promise<void>) | undefined;
afterEach(async () => {
	await close?.();
	close = undefined;
	document.body.replaceChildren();
	document.documentElement.removeAttribute(THEME_ATTRIBUTE);
});

const mount = async (shown: ReturnType<typeof host>) => {
	await page.viewport(1280, 900);
	const root = document.createElement("div");
	document.body.append(root);
	close = await boot(root, shown.transport);
	return root;
};

const theme = () => document.documentElement.getAttribute(THEME_ATTRIBUTE);

// Kills a page that draws its own default theme over the host's, ignores the
// host's later change, draws anything but the call's result, leaves the host
// to guess its height, or lets a link leave the frame by itself rather than
// through the host.
test("draws the call's result in the host's theme, and opens its line through the host", async () => {
	// The browser prefers light, so a page that ignores the host draws light.
	expect(matchMedia("(prefers-color-scheme: light)").matches).toBe(true);
	const shown = host({ theme: "dark", first: success(perf()) });
	const root = await mount(shown);
	expect(theme()).toBe("dark");
	const figure = page.getByRole("figure");
	await expect.element(figure).toHaveAccessibleName("2 lines");
	const images = [...document.querySelectorAll<HTMLImageElement>(".brand img")];
	expect(images.map(({ src }) => src.slice(0, 19))).toEqual([
		"data:image/svg+xml,",
		"data:image/svg+xml,",
	]);
	// The page reports what it draws, not the frame it was given, so the frame can shrink to it.
	const drawn = Math.ceil(root.getBoundingClientRect().height);
	expect(drawn).toBeLessThan(window.innerHeight);
	await expect.poll(() => shown.sizes.at(-1)?.height).toBe(drawn);

	await userEvent.click(page.getByRole("link", { name: "Hashbrown" }));
	await userEvent.click(page.getByRole("link", { name: "Bencher" }));
	await expect.poll(() => shown.opened).toEqual([URL_, URL_]);

	shown.notify("ui/notifications/host-context-changed", { theme: "light" });
	await expect.poll(theme).toBe("light");
});

// Kills a refresh that asks for anything but the call's own query, draws
// nothing new, or shows again the lines the reader hid.
test("refresh calls query_perf with the call's own arguments and keeps the lines the reader hid", async () => {
	const shown = host({
		first: success(perf()),
		refreshes: [success(perf("Hashbrown again"))],
	});
	await mount(shown);
	await expect
		.element(page.getByRole("figure"))
		.toHaveAccessibleName("2 lines");
	await userEvent.click(
		page.getByRole("button", { name: /hide its line/ }).first(),
	);
	await expect
		.element(page.getByRole("button", { name: /show its line/ }))
		.toBeVisible();

	await userEvent.click(page.getByRole("button", { name: "Refresh" }));
	await expect
		.element(page.getByRole("link", { name: "Hashbrown again" }))
		.toBeVisible();
	expect(shown.calls).toEqual([
		expect.objectContaining({ name: "query_perf", arguments: INPUT }),
	]);
	await expect
		.element(page.getByRole("button", { name: /show its line/ }))
		.toBeVisible();
});

// Kills a failed call drawn as a plot on its way, a failure in words of the
// page's own, and a refused refresh that takes the drawn plot away.
test("a call that fails says so in the embed's words, and a drawn plot stays", async () => {
	const shown = host({
		first: toolError("404: Project not found"),
		refreshes: [success(perf()), toolError("429: Too many requests"), null],
	});
	await mount(shown);
	await expect
		.element(page.getByText("The API refused this query"))
		.toBeVisible();
	await expect
		.element(page.getByRole("link", { name: "hashbrown" }))
		.toBeVisible();
	expect(
		page.getByRole("img", { name: "Loading the plot" }).elements(),
	).toHaveLength(0);

	const refresh = page.getByRole("button", { name: "Refresh" });
	await userEvent.click(refresh);
	await expect
		.element(page.getByRole("figure"))
		.toHaveAccessibleName("2 lines");
	await expect
		.element(page.getByText("The API refused this query"))
		.not.toBeInTheDocument();

	// A tool's own refusal, as the API's rate limit answers.
	await expect.element(refresh).toBeEnabled();
	await userEvent.click(refresh);
	await expect
		.element(page.getByText("The API refused this query"))
		.toBeVisible();
	await expect
		.element(page.getByRole("figure"))
		.toHaveAccessibleName("2 lines");

	// A host that could not make the call.
	await expect.element(refresh).toBeEnabled();
	await userEvent.click(refresh);
	await expect.element(page.getByText("The plot did not load")).toBeVisible();
	await expect
		.element(page.getByRole("figure"))
		.toHaveAccessibleName("2 lines");
});

// Kills readout links that stay relative to the frame, so they open on the
// host's origin, links that skip the host, and an alert the page offers to change.
test("a pinned readout links to its report and its alert in the console, through the host", async () => {
	const shown = host({ first: success(perf()) });
	await mount(shown);
	// The key lists the alerted line first, and the readout offers the alert of the line in focus.
	await userEvent.click(
		page.getByRole("button", { name: /^Focus line of / }).first(),
	);
	const area = page.getByRole("group", { name: /lines?,/ });
	const box = area.element().getBoundingClientRect();
	await area.click({ position: { x: box.width / 2, y: box.height / 2 } });

	const alert = page.getByRole("link", { name: /^Alert: / });
	await expect.element(alert).toBeVisible();
	expect(
		page.getByRole("button", { name: /the alert on/ }).elements(),
	).toHaveLength(0);
	await userEvent.click(alert);
	const report = document.querySelector<HTMLAnchorElement>(".pl-when a");
	if (!report) {
		throw new Error("The pinned readout has no report link");
	}
	await userEvent.click(report);
	await expect
		.poll(() => shown.opened)
		.toEqual([
			`${CONSOLE}/console/projects/hashbrown/alerts/${uuid(40)}`,
			`${CONSOLE}/console/projects/hashbrown/reports/${uuid(21)}`,
		]);
});
