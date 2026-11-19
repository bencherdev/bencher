import { afterEach, expect, test, vi } from "vitest";
import { Refusal, reads } from "./read";

afterEach(() => {
	vi.unstubAllGlobals();
});

const answering = (answer: Response) => {
	const asked: string[] = [];
	vi.stubGlobal("fetch", async (input: string) => {
		asked.push(input);
		return answer;
	});
	return asked;
};

// Kills a public read sent anywhere but the console's own relay.
test("reads through the console's own relay", async () => {
	const asked = answering(new Response('{"name":"Hashbrown"}'));
	expect(await reads()("/v0/projects/hashbrown")).toEqual({
		name: "Hashbrown",
	});
	expect(asked).toEqual(["/api/v0/projects/hashbrown"]);
});

// Kills a refusal whose body is left unread: the relay marks every answer
// `no-store`, so the browser holds its connection until the page goes.
test("a refused read drains its body", async () => {
	const answer = new Response('{"message":"Not Found"}', { status: 404 });
	answering(answer);
	const refused = await reads()("/v0/projects/private").catch(
		(error: unknown) => error,
	);
	expect(refused).toBeInstanceOf(Refusal);
	expect((refused as Refusal).status).toBe(404);
	expect(answer.bodyUsed).toBe(true);
});
