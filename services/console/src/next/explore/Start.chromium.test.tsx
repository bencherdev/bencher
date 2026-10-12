import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "./explore.css";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import type { Api } from "../api";
import { mount } from "../settings/testing";
import Start from "./Start";

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
});

const api: Api = {
	get: async <T,>() => ({ data: [] as T, headers: new Headers() }),
	send: async () => {
		throw new Error("The start lists send no changes");
	},
};

// Kills starting points out of the tabs' order, or drawn in an order that
// their reading and keyboard order does not follow.
for (const [width, edge] of [
	[1280, "left"],
	[390, "top"],
] as const) {
	test(`the starting points run Pinned plots, Latest reports, Alerting now at ${width} px`, async () => {
		await page.viewport(width, 844);
		dispose = mount(() => <Start />, { api }).dispose;
		const lists = ["Pinned plots", "Latest reports", "Alerting now"].map(
			(name) => page.getByRole("region", { name }).element(),
		);
		for (const [at, list] of lists.entries()) {
			const next = lists[at + 1];
			if (next) {
				expect(
					list.compareDocumentPosition(next) & Node.DOCUMENT_POSITION_FOLLOWING,
				).toBeTruthy();
				expect(next.getBoundingClientRect()[edge]).toBeGreaterThan(
					list.getBoundingClientRect()[edge],
				);
			}
		}
	});
}

// Kills a link to a whole list too small to tap on a phone.
test("on a phone each list's link to all of it is a 44 px target", async () => {
	await page.viewport(390, 844);
	dispose = mount(() => <Start />, { api }).dispose;
	for (const name of ["All alerts", "All reports", "All plots"]) {
		const link = page.getByRole("link", { name });
		await expect.element(link).toBeVisible();
		expect(
			link.element().getBoundingClientRect().height,
		).toBeGreaterThanOrEqual(44);
	}
});
