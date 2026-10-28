import { readFileSync } from "node:fs";
import {
	type BrowserContext,
	type BrowserContextOptions,
	type Locator,
	test as base,
	expect,
	type Page,
	type Request,
} from "@playwright/test";
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
	/** The API's log, which holds every email it sends. */
	api_log: string;
	now: string;
	admin: JsonAuthUser;
	member: JsonAuthUser;
	/** A member of nothing, signed in with a user key. */
	outsider: JsonAuthUser;
	organization: SeedProject;
	projects: {
		hashbrown: SeedProject;
		/** Version 0, with runs that send bounds, a bare threshold with one alert, and a classic pin. */
		version_zero: SeedProject;
		empty: SeedProject;
		private: SeedProject;
		/** Runs of three iterations on one line, the newest raising an alert on its second. */
		iterations: SeedProject;
		/** Runner jobs that wait, run, failed, and were canceled, and one finished run. */
		runner: SeedProject;
		/** Each with one run that alerted and still runs, which tests add alerting reports to: one per repeat, then one for the axe scan. */
		held: SeedProject[];
		/** Each with one report, a queued job's, on an archived branch: one per repeat. */
		handover: SeedProject[];
	};
	/** The admin's version 1 project with the newest report, in an organization no test changes. */
	admin_newest: SeedProject;
	last_main_hash: string;
	split_hash: string;
	/** The short form of the newest version 0 run's hash. */
	version_zero_hash: string;
	/** The runner project's run whose jobs run and wait, one report landed. */
	jobs_hash: string;
	held_hash: string;
}

const seedPath = process.env.BENCHER_E2E_SEED;
if (!seedPath) {
	throw new Error("Run the end-to-end suite with `cargo test-console e2e`");
}
export const seed: Seed = JSON.parse(readFileSync(seedPath, "utf8"));

export const USER_KEY = "BENCHER_USER";
export const THEME_KEY = "BENCHER_THEME";
export const VERSION_KEY = "BENCHER_BMF_VERSIONS";
export const SHELL_KEY = "BENCHER_CONSOLE_SHELL";
export const ORGANIZATION_KEY = "BENCHER_ORGANIZATION";

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

/** Sign in, carrying `back` as the page it returns to. */
export const logInTo = (back: string) =>
	`/auth/login?${new URLSearchParams({ back: Buffer.from(back).toString("base64") })}`;

export const consolePath = (slug: string, tab = "") =>
	`/console/projects/${slug}/${tab}`;

export const test = base.extend<{
	freezeClock: boolean;
	frozenClock: undefined;
	trackedRequests: undefined;
	/** A browser of its own, at the same date as the test's. */
	newContext: (options?: BrowserContextOptions) => Promise<BrowserContext>;
}>({
	freezeClock: [true, { option: true }],
	// Only the date is frozen, never the timers. Playwright's clock fakes the
	// timers of every frame, each counting its own ids, so a timer set through
	// a sandbox frame (as Sentry does) and cleared through the page clears the
	// page's own timer of that id, such as the cache's next save.
	// Frozen for the whole context, so a second page of the test's sees the seed's date too.
	frozenClock: [
		async ({ context, freezeClock }, use) => {
			if (freezeClock) {
				await context.addInitScript(freezeDate, Date.parse(seed.now));
			}
			await use(undefined);
		},
		{ auto: true },
	],
	// Counted from each page's start, so `settle` sees a request already in flight when it is called.
	trackedRequests: [
		async ({ context }, use) => {
			trackPages(context);
			await use(undefined);
		},
		{ auto: true },
	],
	newContext: async ({ browser, freezeClock }, use) => {
		const contexts: BrowserContext[] = [];
		await use(async (options) => {
			const context = await browser.newContext(options);
			contexts.push(context);
			trackPages(context);
			if (freezeClock) {
				await context.addInitScript(freezeDate, Date.parse(seed.now));
			}
			return context;
		});
		await Promise.all(contexts.map((context) => context.close()));
	},
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

/** The Org / Project crumb, which opens the switcher, by the start of its name. */
export const crumb = (page: Page, name = "") =>
	page
		.getByRole("banner")
		.getByRole("button", { name: `${name}, switch project` });

/** Each page's requests in flight, and when the last one ended. */
const traffic = new WeakMap<Page, Traffic>();

interface Traffic {
	inFlight: Set<Request>;
	/** The main frame's request for a new document, until the document commits. */
	navigation?: Request;
	last: number;
}

const track = (page: Page) => {
	const known = traffic.get(page);
	if (known) {
		return known;
	}
	const counted: Traffic = { inFlight: new Set(), last: Date.now() };
	traffic.set(page, counted);
	const end = (request: Request) => {
		if (counted.inFlight.delete(request)) {
			counted.last = Date.now();
		}
	};
	page.on("request", (request) => {
		if (request.isNavigationRequest() && request.frame() === page.mainFrame()) {
			counted.navigation = request;
		}
		counted.inFlight.add(request);
	});
	page.on("requestfinished", end);
	page.on("requestfailed", (request) => {
		if (request === counted.navigation) {
			counted.navigation = undefined;
		}
		end(request);
	});
	// A new document's commit ends the old one's requests, and no event says so.
	page.on("framenavigated", (frame) => {
		const { navigation } = counted;
		if (
			frame !== page.mainFrame() ||
			!navigation ||
			withoutHash(frame.url()) !== withoutHash(navigation.url())
		) {
			return;
		}
		counted.navigation = undefined;
		for (const request of counted.inFlight) {
			if (request !== navigation) {
				end(request);
			}
		}
	});
	return counted;
};

const withoutHash = (url: string) => url.split("#")[0];

const trackPages = (context: BrowserContext) => {
	for (const page of context.pages()) {
		track(page);
	}
	context.on("page", track);
};

/** Resolves once no request has been in flight for `quietMs`, counting from the call at the earliest. */
export const settle = async (page: Page, quietMs = 300) => {
	const counted = track(page);
	const called = Date.now();
	await expect
		.poll(
			() =>
				counted.inFlight.size === 0 &&
				Date.now() - Math.max(counted.last, called) >= quietMs,
			{ intervals: [50] },
		)
		.toBe(true);
};

/** Moves the pointer across `lines` as a reader heading for a row does, which brings the rows' Explore links without waiting for the page to go idle. */
export const approach = async (lines: Locator) => {
	await lines.scrollIntoViewIfNeeded();
	const box = await lines.boundingBox();
	if (!box) {
		throw new Error("The lines are not drawn");
	}
	const top = Math.max(box.y, 0);
	const mouse = lines.page().mouse;
	await mouse.move(box.x + 8, top + 8);
	await mouse.move(box.x + box.width / 2, top + 16, { steps: 4 });
};

/** How many queries the signed in reader has persisted. */
export const persistedQueries = (page: Page) =>
	page.evaluate(
		(uuid) =>
			new Promise<number>((resolve) => {
				const open = indexedDB.open("bencher-console");
				open.onerror = () => resolve(0);
				open.onsuccess = () => {
					const db = open.result;
					if (!db.objectStoreNames.contains("cache")) {
						db.close();
						return resolve(0);
					}
					const get = db.transaction("cache").objectStore("cache").get(uuid);
					get.onsuccess = () => {
						db.close();
						resolve(get.result?.state?.queries?.length ?? 0);
					};
					get.onerror = () => {
						db.close();
						resolve(0);
					};
				};
			}),
		seed.member.user.uuid,
	);
