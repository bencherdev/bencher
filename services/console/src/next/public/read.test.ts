import { afterEach, expect, test, vi } from "vitest";
import { Refusal, reads } from "./read";

afterEach(() => {
	vi.unstubAllGlobals();
});

const answering = (answer: () => Response) => {
	const asked: { url: string; credentials: RequestCredentials | undefined }[] =
		[];
	vi.stubGlobal("fetch", async (input: string, init?: RequestInit) => {
		asked.push({ url: input, credentials: init?.credentials });
		return answer();
	});
	return asked;
};

// Kills a public read sent anywhere but the console's own relay, one that
// carries the session when it reads as no one, and one that leaves it behind
// when it reads as the reader.
test("reads through the console's own relay, as no one or as the reader", async () => {
	const asked = answering(() => new Response('{"name":"Hashbrown"}'));
	expect(await reads()("/v0/projects/hashbrown")).toEqual({
		name: "Hashbrown",
	});
	expect(await reads(true)("/v0/projects/hashbrown")).toEqual({
		name: "Hashbrown",
	});
	expect(asked).toEqual([
		{ url: "/api/v0/projects/hashbrown", credentials: "omit" },
		{ url: "/api/v0/projects/hashbrown", credentials: "same-origin" },
	]);
});

// Kills a refusal whose body is left unread: the relay marks every answer
// `no-store`, so the browser holds its connection until the page goes.
test("a refused read drains its body", async () => {
	const answer = new Response('{"message":"Not Found"}', { status: 404 });
	answering(() => answer);
	const refused = await reads()("/v0/projects/private").catch(
		(error: unknown) => error,
	);
	expect(refused).toBeInstanceOf(Refusal);
	expect((refused as Refusal).status).toBe(404);
	expect(answer.bodyUsed).toBe(true);
});
