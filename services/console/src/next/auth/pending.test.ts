import { describe, expect, test } from "vitest";
import {
	doorPath,
	forgetPending,
	keepPending,
	keepPrefill,
	pendingOf,
	readPending,
	takePrefill,
} from "./pending";

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

const PENDING = {
	email: "ada@example.com",
	door: "signup" as const,
	carry: { plan: "pro", authorization: "0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f" },
	resendAt: 1_000,
	cappedUntil: 2_000,
};

describe("the pending sign-in", () => {
	// Kills a tab that loses the address it emailed, the door it began at, what
	// it carries, or when Resend opens, and one that keeps it after it ends.
	test("is kept for the tab until forgotten", () => {
		const storage = memory();
		keepPending(storage, PENDING);
		expect(readPending(storage)).toEqual(PENDING);
		forgetPending(storage);
		expect(readPending(storage)).toBeUndefined();
	});

	// Kills a tab that takes anything written there for a sign-in.
	test("a broken record is none", () => {
		for (const kept of [
			"{",
			"null",
			JSON.stringify({ ...PENDING, email: "" }),
			JSON.stringify({ ...PENDING, door: "elsewhere" }),
		]) {
			expect(
				readPending(memory({ BENCHER_PENDING_SIGN_IN: kept })),
				kept,
			).toBeUndefined();
		}
		expect(
			readPending(
				memory({
					BENCHER_PENDING_SIGN_IN: JSON.stringify({
						...PENDING,
						carry: { plan: 7, claim: "" },
						resendAt: "soon",
					}),
				}),
			),
		).toEqual({
			email: PENDING.email,
			door: PENDING.door,
			carry: {},
			cappedUntil: PENDING.cappedUntil,
		});
	});
});

describe("pendingOf", () => {
	// Kills a Resend that opens at the wrong time, and a cap's wait that is lost
	// or taken for Resend's.
	test("dates Resend and the cap from the API's seconds", () => {
		expect(
			pendingOf(
				{ email: "ada@example.com", resend_after: 60 },
				"login",
				{},
				1_000,
			),
		).toEqual({
			email: "ada@example.com",
			door: "login",
			carry: {},
			resendAt: 61_000,
		});
		expect(
			pendingOf({ email: "ada@example.com", wait: 2_400 }, "signup", {}, 1_000),
		).toEqual({
			email: "ada@example.com",
			door: "signup",
			carry: {},
			cappedUntil: 2_401_000,
		});
	});
});

describe("the door's address", () => {
	// Kills a sign-in begun again that drops what the first carried.
	test("carries what the sign-in carried", () => {
		expect(doorPath("login", {})).toBe("/auth/login");
		expect(
			doorPath("signup", {
				invite: "a.b.c",
				claim: "c1a1d000-0000-4000-8000-000000000000",
				plan: "pro",
				authorization: PENDING.carry.authorization,
			}),
		).toBe(
			`/auth/signup?invite=a.b.c&claim=c1a1d000-0000-4000-8000-000000000000&plan=pro&authorization=${PENDING.carry.authorization}`,
		);
	});

	// Kills an address Use another email loses, or fills in again later.
	test("fills in the address it took back once", () => {
		const storage = memory();
		keepPrefill(storage, "ada@example.com");
		expect(takePrefill(storage)).toBe("ada@example.com");
		expect(takePrefill(storage)).toBeUndefined();
	});
});
