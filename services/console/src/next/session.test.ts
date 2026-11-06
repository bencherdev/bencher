import { expect, test } from "vitest";
import { endSession } from "./session";

// Kills a sign out that ends no session on the API, one that ends it through
// anything but the console's own relay, and one that leaves the answer unread,
// which holds its connection.
test("ends the session through the relay and reads the answer", async () => {
	const sent: { url: string; method: string | undefined }[] = [];
	const answers: Response[] = [];
	await endSession(async (url, init) => {
		sent.push({ url: String(url), method: init?.method });
		const answer = new Response('{"message":"Invalid credential"}', {
			status: 401,
		});
		answers.push(answer);
		return answer;
	});
	expect(sent).toEqual([{ url: "/api/v0/auth/session", method: "DELETE" }]);
	expect(answers.every((answer) => answer.bodyUsed)).toBe(true);
});

// Kills a sign out stuck on an API that refuses or cannot be reached.
test("a refused or unreachable API still lets the reader go", async () => {
	await expect(
		endSession(async () => new Response("{}", { status: 401 })),
	).resolves.toBeUndefined();
	await expect(
		endSession(async () => {
			throw new TypeError("Failed to fetch");
		}),
	).resolves.toBeUndefined();
});
