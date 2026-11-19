import { expect, test } from "vitest";
import { consentPath } from "../../components/auth/authorize/path";
import { PlanLevel } from "../../types/bencher";
import { READER_KEY } from "../reader";
import { finish, landing } from "./land";

const REQUEST = "0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f";
const BACK = "https://bencher.dev/console/projects/hashbrown/alerts";

// Kills a sign-in that lands anywhere but an app's pending consent first, the
// page the reader asked for next, and the console with the plan they chose
// last, where the console onboards a new account.
test("lands on a consent, then the page asked for, then the console", () => {
	const kept = (page: undefined | string) => (authorization: unknown) =>
		consentPath(authorization) ?? page;
	expect(landing({ authorization: REQUEST }, kept(BACK))).toBe(
		consentPath(REQUEST),
	);
	expect(landing({}, kept(BACK))).toBe(BACK);
	expect(landing({ plan: PlanLevel.Pro }, kept(undefined))).toBe(
		"/console?plan=pro",
	);
	expect(landing({}, kept(undefined))).toBe("/console");
});

/** Storage in memory, as a test hands a page's. */
const memory = (entries: Record<string, string> = {}) => ({
	entries,
	getItem: (key: string) => entries[key] ?? null,
	setItem: (key: string, value: string) => {
		entries[key] = value;
	},
	removeItem: (key: string) => {
		delete entries[key];
	},
});

// Kills a sign-in that keeps more of the reader than the hint, leaves the
// user and session token the console stored before the cookie, keeps this
// tab's pending sign-in, or lands anywhere but where a sign-in goes.
test("a finished sign-in keeps the hint alone and lands", () => {
	const local = memory({ BENCHER_USER: '{"token":"tok"}' });
	const session = memory({ BENCHER_PENDING_SIGN_IN: "{}" });
	const went: string[] = [];
	finish(
		{
			user: {
				uuid: "u",
				slug: "muriel",
				name: "Muriel",
				email: "muriel@nowhere.example",
				admin: false,
				locked: false,
			},
			new: false,
			plan: PlanLevel.Pro,
		},
		{ local, session, go: (to) => went.push(to), kept: () => undefined },
	);
	expect(local.entries).toEqual({
		[READER_KEY]: JSON.stringify({ uuid: "u", slug: "muriel", name: "Muriel" }),
	});
	expect(session.entries).toEqual({});
	expect(went).toEqual(["/console?plan=pro"]);
});
