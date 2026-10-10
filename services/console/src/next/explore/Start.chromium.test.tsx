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
