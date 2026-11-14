import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { DatabaseSync } from "node:sqlite";
import {
	type BrowserContext,
	type BrowserContextOptions,
	type Locator,
	test as base,
	expect,
	type Page,
	type Request,
} from "@playwright/test";
import type { JsonUser } from "../src/types/bencher";

interface SeedProject {
	uuid: string;
	name: string;
	slug: string;
}

/** A reader the suite signs in as: a real session for the browser, and a user key for the suite's own requests. */
export interface Reader {
	user: JsonUser;
	/** The value of the session cookie a sign-in by email set. */
	session: string;
	key: string;
}

/** What `cargo test-console e2e` seeded the API with. */
export interface Seed {
	api_url: string;
	console_url: string;
	/** A second console, whose relay secret is not the API's, and which reaches the API only through `INTERNAL_API_URL`. */
	wrong_secret_console_url: string;
	/** The API's log, which holds every email it sends. */
	api_log: string;
	/** The API's database, where the suite sets when a session last proved its reader. */
	db: string;
	now: string;
	admin: Reader;
	member: Reader;
	/** A member of nothing. */
	outsider: Reader;
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

/** Where the browser reaches the API: the console's own relay. */
export const RELAY = `${seed.console_url}/api`;

/** The reader hint, which names who signed in and authenticates nothing. */
export const READER_KEY = "BENCHER_READER";
export const SESSION_COOKIE = "__Host-bencher_session";
export const THEME_KEY = "BENCHER_THEME";
export const VERSION_KEY = "BENCHER_BMF_VERSIONS";
export const SHELL_KEY = "BENCHER_CONSOLE_SHELL";
export const ORGANIZATION_KEY = "BENCHER_ORGANIZATION";

type StorageState = {
	cookies: {
		name: string;
		value: string;
		domain: string;
		path: string;
		expires: number;
		httpOnly: boolean;
		secure: boolean;
		sameSite: "Strict";
	}[];
	origins: {
		origin: string;
		localStorage: { name: string; value: string }[];
	}[];
};

/** The session cookie, as the API sets it for the suite's console. */
export const sessionCookie = (session: string) => ({
	name: SESSION_COOKIE,
	value: session,
	domain: new URL(seed.console_url).hostname,
	path: "/",
	expires: -1,
	httpOnly: true,
	secure: true,
	sameSite: "Strict" as const,
});

/** What the console keeps of a reader in the browser: who they are, and nothing that signs in. */
export const hintOf = ({ user }: Pick<Reader, "user">) =>
	JSON.stringify({
		uuid: user.uuid,
		slug: user.slug,
		name: user.name,
		...(user.admin ? { admin: true } : {}),
	});

/** A browser signed in as `reader`, with its session cookie and its hint, plus any other entries. */
export const signedIn = (
	reader: Reader,
	extra: Record<string, string> = {},
): StorageState => ({
	cookies: [sessionCookie(reader.session)],
	origins: [
		{
			origin: seed.console_url,
			localStorage: [
				{ name: READER_KEY, value: hintOf(reader) },
				...Object.entries(extra).map(([name, value]) => ({ name, value })),
			],
		},
	],
});

export const signedOut: StorageState = { cookies: [], origins: [] };

/** `reader` with a session cookie the API never began, as a browser holds one after its session ends. */
export const ended = (reader: Reader): Reader => ({
	...reader,
	session: Buffer.alloc(32, 0xfb).toString("base64url"),
});

/** Sign in, carrying `back` as the page it returns to. */
export const logInTo = (back: string) =>
	`/auth/login?${new URLSearchParams({ back: Buffer.from(back).toString("base64") })}`;

export const consolePath = (slug: string, tab = "") =>
	`/console/projects/${slug}/${tab}`;

/**
 * Set when the session `session` names last proved its reader, `seconds` ago,
 * as the API records a sign-in: the suite's own clock for the window that
 * guards the sensitive actions, which no request can turn back. Only a test's
 * own reader ages, since the seeded readers' sessions serve every worker at
 * once.
 */
export const provedAgo = (session: string, seconds: number) => {
	if (seeded().includes(session)) {
		throw new Error(
			"A seeded reader's session serves every test: age a reader of the test's own",
		);
	}
	setProved(session, seconds);
};

const seeded = () =>
	[seed.admin, seed.member, seed.outsider].map(({ session }) => session);

const setProved = (session: string, seconds: number) => {
	const db = new DatabaseSync(seed.db);
	try {
		db.exec("PRAGMA busy_timeout = 10000");
		db.prepare("UPDATE session SET proved = ? WHERE token_hash = ?").run(
			Math.floor(Date.now() / 1_000) - seconds,
			createHash("sha256").update(session).digest("hex"),
		);
	} finally {
		db.close();
	}
};

export const test = base.extend<{
	freezeClock: boolean;
	/** The seeded readers signed in moments ago, as every test but the fresh sign-in's assumes. */
	seedProved: undefined;
	turnstile: undefined;
	frozenClock: undefined;
	trackedRequests: undefined;
	/** Every request a page sent off its console's origin. */
	strays: string[];
	/** Every request a page sent with an `Authorization` header. */
	authorized: string[];
	/** A browser of its own, at the same date as the test's. */
	newContext: (options?: BrowserContextOptions) => Promise<BrowserContext>;
}>({
	freezeClock: [true, { option: true }],
	// The suite runs longer than the 10 minutes a sign-in lets a session make a key or delete,
	// so each test starts with the seeded sessions proved now.
	seedProved: [
		async ({ browserName: _ }, use) => {
			for (const session of seeded()) {
				setProved(session, 0);
			}
			await use(undefined);
		},
		{ auto: true },
	],
	// The console asks Cloudflare's Turnstile for a token, as Bencher Cloud's does; the suite
	// answers in its place, refusing where a test sets `TURNSTILE_REFUSES`.
	turnstile: [
		async ({ context }, use) => {
			await fakeTurnstile(context);
			await use(undefined);
		},
		{ auto: true },
	],
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
	// Kills a page that reads the API anywhere but through the relay on its own origin.
	strays: [
		async ({ context }, use) => {
			const strays: string[] = [];
			watchOrigin(context, strays);
			await use(strays);
			expect(strays, "requests that left the console's origin").toEqual([]);
		},
		{ auto: true },
	],
	// Kills a page that sends a credential of its own: the session rides in its cookie alone.
	authorized: [
		async ({ context }, use) => {
			const authorized: string[] = [];
			watchAuthorization(context, authorized);
			await use(authorized);
			expect(
				authorized,
				"requests that carried an Authorization header",
			).toEqual([]);
		},
		{ auto: true },
	],
	newContext: async ({ browser, freezeClock, strays, authorized }, use) => {
		const contexts: BrowserContext[] = [];
		await use(async (options) => {
			const context = await browser.newContext(options);
			contexts.push(context);
			await fakeTurnstile(context);
			trackPages(context);
			watchOrigin(context, strays);
			watchAuthorization(context, authorized);
			if (freezeClock) {
				await context.addInitScript(freezeDate, Date.parse(seed.now));
			}
			return context;
		});
		await Promise.all(contexts.map((context) => context.close()));
	},
});

/** Cloudflare's Turnstile script, as the auth pages load it. */
export const TURNSTILE = "https://challenges.cloudflare.com/turnstile/**";
/** Set in a page, the suite's Turnstile refuses every check. */
export const TURNSTILE_REFUSES = "BENCHER_E2E_TURNSTILE_REFUSES";

const fakeTurnstile = (context: BrowserContext) =>
	context.route(TURNSTILE, (route) =>
		route.fulfill({
			contentType: "text/javascript",
			body: `window.turnstile = {
	drawn: 0,
	widgets: {},
	render(container, options) {
		this.drawn += 1;
		const id = String(this.drawn);
		this.widgets[id] = options;
		return id;
	},
	execute(id) {
		const options = this.widgets[id];
		setTimeout(() =>
			window[${JSON.stringify(TURNSTILE_REFUSES)}]
				? options["error-callback"]()
				: options.callback("e2e-turnstile-token-" + id),
		);
	},
	remove(id) {
		delete this.widgets[id];
	},
};`,
		}),
	);

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

const CONSOLES = new Set(
	[seed.console_url, seed.wrong_secret_console_url].map(
		(url) => new URL(url).origin,
	),
);
const API_ORIGIN = new URL(seed.api_url).origin;
/** Requests a page makes from script, which a strict `connect-src 'self'` covers. */
const CONNECT = new Set(["fetch", "xhr", "eventsource", "websocket"]);
/** Where a console page sends what is not the API's: the suite's Sentry DSN, and Google Analytics until no console page loads it. */
const THIRD_PARTIES = [
	(url: URL) => url.origin === "http://127.0.0.1:9",
	(url: URL) =>
		/(^|\.)(google-analytics\.com|analytics\.google\.com|googletagmanager\.com)$/.test(
			url.hostname,
		),
];

/**
 * Records every request a page in `context` sends to the API's own origin,
 * but for an outside app sending the browser's top frame to the API's OAuth
 * pages, and every request a console page's script sends off its origin to
 * anyone but a third party it loads.
 */
const watchOrigin = (context: BrowserContext, strays: string[]) => {
	context.on("request", (request) => {
		const url = new URL(request.url());
		if (url.origin === API_ORIGIN) {
			if (!(isTopNavigation(request) && url.pathname.startsWith("/oauth/"))) {
				strays.push(request.url());
			}
			return;
		}
		if (
			request.isNavigationRequest() ||
			!CONNECT.has(request.resourceType()) ||
			CONSOLES.has(url.origin) ||
			THIRD_PARTIES.some((third) => third(url))
		) {
			return;
		}
		let from: string;
		try {
			from = new URL(request.frame().url()).origin;
		} catch {
			return;
		}
		if (CONSOLES.has(from)) {
			strays.push(request.url());
		}
	});
};

/** Records every request a page in `context` sends with an `Authorization` header. */
const watchAuthorization = (context: BrowserContext, authorized: string[]) => {
	context.on("request", (request) => {
		if (request.headers().authorization !== undefined) {
			authorized.push(request.url());
		}
	});
};

const isTopNavigation = (request: Request) => {
	try {
		return (
			request.isNavigationRequest() && request.frame().parentFrame() === null
		);
	} catch {
		return false;
	}
};

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

/** How many queries the signed in reader, the member unless named, has persisted. */
export const persistedQueries = (page: Page, uuid = seed.member.user.uuid) =>
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
		uuid,
	);
