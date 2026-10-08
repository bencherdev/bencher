import { QueryClient, dehydrate, hydrate } from "@tanstack/solid-query";
import { expect, test } from "vitest";
import type { Api } from "./api";
import { consoleProjectQuery, fetchedVersions } from "./queries";

const answering = (version: number) => {
	let answer: () => void = () => {};
	const api = {
		get: () =>
			new Promise((resolve) => {
				answer = () => resolve({ data: { project: { bmf_version: version } } });
			}),
	} as unknown as Api;
	return { api, answer: () => answer() };
};

/** What a cache saved when `hashbrown` was still on BMF version 0. */
const savedAtVersionZero = async () => {
	const client = new QueryClient();
	const { api, answer } = answering(0);
	const fetching = client.query(consoleProjectQuery(api, "hashbrown"));
	answer();
	await fetching;
	return dehydrate(client);
};

// Kills taking a restored response for one fetched on this page load.
test("only a response fetched on this load settles a project's version", async () => {
	const saved = await savedAtVersionZero();
	const client = new QueryClient();
	const versions = fetchedVersions(client);
	const { api, answer } = answering(1);

	// Boot starts the request, then restores the cache while it is in flight.
	const fetching = client.query(consoleProjectQuery(api, "hashbrown"));
	hydrate(client, saved);
	expect(
		client.getQueryData(["console", "project", "hashbrown"]),
	).toMatchObject({ project: { bmf_version: 0 } });
	expect(versions()).toEqual({});

	answer();
	await fetching;
	expect(versions()).toEqual({ hashbrown: 1 });
});

// Kills a version taken from the console's own writes to the cache, such as
// an optimistic rename built on a restored response.
test("a write to the cache settles no version", () => {
	const client = new QueryClient();
	const versions = fetchedVersions(client);
	client.setQueryData(["console", "project", "hashbrown"], {
		project: { bmf_version: 0 },
	});
	expect(versions()).toEqual({});
});
