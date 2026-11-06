import { describe, expect, test } from "vitest";
import { decodeBase64 } from "../util/base64";
import { READER_KEY, readReader, rememberReader, signInHref } from "./reader";

const user = {
	uuid: "8c5b2b6e-4f3a-4c2d-9e1f-0a1b2c3d4e5f",
	name: "Muriel Bagge",
	slug: "muriel-bagge",
	email: "muriel.bagge@nowhere.com",
	admin: false,
	locked: false,
};
const hint = { uuid: user.uuid, slug: user.slug, name: user.name };

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

describe("readReader", () => {
	// Kills a console that treats a missing, broken, or partial hint as signed in.
	test("reads only a whole hint", () => {
		expect(readReader(memory({ [READER_KEY]: JSON.stringify(hint) }))).toEqual(
			hint,
		);
		for (const stored of [
			undefined,
			"{",
			"null",
			JSON.stringify({ ...hint, uuid: "" }),
			JSON.stringify({ ...hint, slug: undefined }),
			JSON.stringify({ ...hint, name: 7 }),
		]) {
			const storage = memory(
				stored === undefined ? {} : { [READER_KEY]: stored },
			);
			expect(readReader(storage), String(stored)).toBeUndefined();
		}
	});

	// Kills a hint read with anything beyond who the reader is.
	test("keeps only the UUID, slug, and name", () => {
		expect(
			readReader(
				memory({ [READER_KEY]: JSON.stringify({ ...hint, token: "tok" }) }),
			),
		).toEqual(hint);
	});
});

describe("rememberReader", () => {
	// Kills a hint that stores the email, any secret, or the rest of the user.
	test("stores the UUID, slug, and name alone", () => {
		const storage = memory({});
		rememberReader(storage, { ...user, token: "tok" } as typeof user);
		expect(storage.entries).toEqual({ [READER_KEY]: JSON.stringify(hint) });
	});
});

describe("signInHref", () => {
	// Kills a way back that the door cannot read.
	test("carries the way back the door reads", () => {
		const path = "/console/projects/hashbrown/reports?page=2#top";
		const href = new URL(signInHref(path), "https://bencher.dev");
		expect(href.pathname).toBe("/auth/login");
		expect(decodeBase64(href.searchParams.get("back"))).toBe(path);
	});
});
