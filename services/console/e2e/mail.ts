import { randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import type { APIRequestContext } from "@playwright/test";
import type { JsonAuthUser } from "../src/types/bencher";
import { unique } from "./api";
import { expect, seed } from "./fixtures";

/** The link in the newest email the API sent to `email` with `subject`. */
export const mailedLink = async (email: string, subject: string) => {
	let link: undefined | URL;
	await expect
		.poll(() => {
			link = newestLink(email, subject);
			return link;
		})
		.toBeDefined();
	return link as URL;
};

/** A user of the test's own, signed in with a session from the signup email. */
export const signUp = async (
	request: APIRequestContext,
	{
		name = unique("Le Quack"),
		email = `${randomUUID()}@nowhere.com`,
	}: { name?: string; email?: string } = {},
): Promise<JsonAuthUser> => {
	const signup = await request.post(`${seed.api_url}/v0/auth/signup`, {
		data: { name, email, i_agree: true },
	});
	expect(signup.status()).toBe(202);
	const link = await mailedLink(email, "Confirm Bencher Signup");
	const confirm = await request.post(`${seed.api_url}/v0/auth/confirm`, {
		data: { token: link.searchParams.get("token") },
	});
	expect(confirm.status()).toBe(200);
	return (await confirm.json()) as JsonAuthUser;
};

// The API logs each email it sends as one JSON line, its first link the email's button.
const newestLink = (email: string, subject: string) => {
	const lines = readFileSync(seed.api_log, "utf8").split("\n").reverse();
	for (const line of lines) {
		const msg = message(line);
		if (msg?.includes(`<${email}>`) && msg.includes(`Subject: ${subject}\n`)) {
			const url = msg.match(/https?:\/\/\S+/)?.[0];
			if (url) {
				return new URL(url);
			}
		}
	}
	return undefined;
};

const message = (line: string): undefined | string => {
	try {
		const msg = JSON.parse(line)?.msg;
		return typeof msg === "string" ? msg : undefined;
	} catch {
		return undefined;
	}
};
