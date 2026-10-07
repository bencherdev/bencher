import { readFileSync } from "node:fs";
import { test as base, expect, type Page } from "@playwright/test";
import type { JsonAuthUser } from "../src/types/bencher";

interface SeedProject {
	uuid: string;
	name: string;
	slug: string;
}

/** What `cargo test-console e2e` seeded the API with. */
export interface Seed {
	api_url: string;
	console_url: string;
	now: string;
	admin: JsonAuthUser;
	member: JsonAuthUser;
	organization: SeedProject;
	projects: {
		hashbrown: SeedProject;
		version_zero: SeedProject;
		empty: SeedProject;
		private: SeedProject;
	};
	last_main_hash: string;
}

const seedPath = process.env.BENCHER_E2E_SEED;
if (!seedPath) {
	throw new Error("Run the end-to-end suite with `cargo test-console e2e`");
}
export const seed: Seed = JSON.parse(readFileSync(seedPath, "utf8"));

export const USER_KEY = "BENCHER_USER";
export const THEME_KEY = "BENCHER_THEME";
export const VERSION_KEY = "BENCHER_BMF_VERSIONS";

type StorageState = {
	cookies: [];
	origins: {
		origin: string;
		localStorage: { name: string; value: string }[];
	}[];
};

/** A browser that has stored `user` as signed in, plus any other entries. */
export const signedIn = (
	user: JsonAuthUser,
	extra: Record<string, string> = {},
): StorageState => ({
	cookies: [],
	origins: [
		{
			origin: seed.console_url,
			localStorage: [
				{ name: USER_KEY, value: JSON.stringify(user) },
				...Object.entries(extra).map(([name, value]) => ({ name, value })),
			],
		},
	],
});

export const signedOut: StorageState = { cookies: [], origins: [] };

export const nextPath = (slug: string, tab = "") =>
	`/next/console/projects/${slug}/${tab}`;

export const test = base.extend<{
	freezeClock: boolean;
	frozenClock: undefined;
}>({
	freezeClock: [true, { option: true }],
	// Only the date is frozen, never the timers. Playwright's clock fakes the
	// timers of every frame, each counting its own ids, so a timer set through
	// a sandbox frame (as Sentry does) and cleared through the page clears the
	// page's own timer of that id, such as the cache's next save.
	frozenClock: [
		async ({ page, freezeClock }, use) => {
			if (freezeClock) {
				await page.addInitScript(freezeDate, Date.parse(seed.now));
			}
			await use(undefined);
		},
		{ auto: true },
	],
});

/** Runs in the page: the current time is `now`, and every other date is itself. */
const freezeDate = (now: number) => {
	const RealDate = Date;
	globalThis.Date = new Proxy(RealDate, {
		construct: (target, args, newTarget) =>
			Reflect.construct(target, args.length > 0 ? args : [now], newTarget),
		apply: () => new RealDate(now).toString(),
		get: (target, property, receiver) =>
			property === "now" ? () => now : Reflect.get(target, property, receiver),
	});
};

export { expect };

export const tabRow = (page: Page) =>
	page.getByRole("navigation", { name: "Project" });

export const crumbs = (page: Page) =>
	page.getByRole("navigation", { name: "Breadcrumb" });

/** Resolves once no request has been in flight for `quietMs`. */
export const settle = async (page: Page, quietMs = 300) => {
	let inFlight = 0;
	let last = Date.now();
	const start = () => {
		inFlight += 1;
	};
	const end = () => {
		inFlight -= 1;
		last = Date.now();
	};
	page.on("request", start);
	page.on("requestfinished", end);
	page.on("requestfailed", end);
	try {
		await expect
			.poll(() => inFlight <= 0 && Date.now() - last >= quietMs, {
				intervals: [50],
			})
			.toBe(true);
	} finally {
		page.off("request", start);
		page.off("requestfinished", end);
		page.off("requestfailed", end);
	}
};
