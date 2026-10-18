import { QueryObserver, focusManager } from "@tanstack/solid-query";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { Api } from "../api";
import { createQueryClient } from "../cache";
import { runReportsQuery, startQueries } from "../explore/start";
import { batchQuery as reportBatchQuery } from "../report/query";
import { parseReportView } from "../report/view";
import { DEFAULT_SEARCH } from "../reports/search";
import { batchQuery } from "./query";
import { runBatchQuery } from "./run";
import { POLL } from "./status";

const RUNNING = { queued: 0, running: 1, failed: 0, canceled: 0, since: 0 };
const ENDED = { queued: 0, running: 0, failed: 1, canceled: 0 };
const JOB = { status: "running", created: 0 };

/** An API whose answers show a job running until `ended()`, counting what is asked. */
const api = (ended: () => boolean) => {
	let asked = 0;
	const jobs = () => (ended() ? ENDED : RUNNING);
	const reading: Api = {
		get: async <T>(path: string) => {
			asked += 1;
			const data = path.includes("/reports/")
				? { job: ended() ? undefined : JOB }
				: path.includes("/runs?")
					? { total: 1, runs: [{ jobs: jobs() }] }
					: { jobs: jobs() };
			return { data: data as T, headers: new Headers() };
		},
		send: async () => {
			throw new Error("A poll sends nothing");
		},
	};
	return { api: reading, asked: () => asked };
};

const SURFACES: [string, (api: Api) => object][] = [
	[
		"the Runs list",
		(api) =>
			batchQuery(api, "hashbrown", DEFAULT_SEARCH, { page: 1, perPage: 8 }),
	],
	[
		"a run page",
		(api) => runBatchQuery(api, "hashbrown", "run", { page: 1, perPage: 8 }),
	],
	[
		"a report page",
		(api) =>
			reportBatchQuery(
				api,
				"hashbrown",
				"report",
				parseReportView(new URLSearchParams()),
				{
					page: 1,
					perPage: 8,
				},
			),
	],
	["blank Explore's latest runs", (api) => startQueries(api, "hashbrown").runs],
	["an opened latest run", (api) => runReportsQuery(api, "hashbrown", "run")],
];

beforeEach(() => {
	vi.useFakeTimers();
});

afterEach(() => {
	focusManager.setFocused(undefined);
	vi.useRealTimers();
});

for (const [surface, query] of SURFACES) {
	// Kills a page that never asks again while a job runs, keeps asking once
	// nothing is unfinished, or keeps asking from a tab in the background.
	test(`${surface} asks again while a job runs, only in view, and stops once none does`, async () => {
		let ended = false;
		const reading = api(() => ended);
		const client = createQueryClient(() => {});
		const observer = new QueryObserver(
			client,
			client.defaultQueryOptions(query(reading.api) as never),
		);
		const stop = observer.subscribe(() => {});
		await vi.advanceTimersByTimeAsync(10);
		expect(reading.asked()).toBeGreaterThanOrEqual(1);
		const first = reading.asked();
		await vi.advanceTimersByTimeAsync(POLL);
		expect(reading.asked()).toBe(first + 1);

		focusManager.setFocused(false);
		await vi.advanceTimersByTimeAsync(3 * POLL);
		expect(reading.asked()).toBe(first + 1);
		focusManager.setFocused(true);
		await vi.advanceTimersByTimeAsync(POLL);
		const resumed = reading.asked();
		expect(resumed).toBeGreaterThan(first + 1);

		ended = true;
		await vi.advanceTimersByTimeAsync(POLL);
		const last = reading.asked();
		await vi.advanceTimersByTimeAsync(3 * POLL);
		expect(reading.asked()).toBe(last);
		stop();
		client.clear();
	});
}

// Kills a later batch of a run page that asks again while only another batch's
// reports run, which repeats the run's whole read, or never asks while its own do.
test("a run page's later batch asks again only while its own reports run", async () => {
	let ended = false;
	let asked = 0;
	const reading: Api = {
		get: async <T>() => {
			asked += 1;
			const reports = [{ job: ended ? undefined : JOB }];
			return { data: { jobs: RUNNING, reports } as T, headers: new Headers() };
		},
		send: async () => {
			throw new Error("A poll sends nothing");
		},
	};
	const client = createQueryClient(() => {});
	const observer = new QueryObserver(
		client,
		client.defaultQueryOptions(
			runBatchQuery(reading, "hashbrown", "run", {
				page: 2,
				perPage: 8,
			}) as never,
		),
	);
	const stop = observer.subscribe(() => {});
	await vi.advanceTimersByTimeAsync(10);
	const first = asked;
	await vi.advanceTimersByTimeAsync(POLL);
	expect(asked).toBe(first + 1);

	ended = true;
	await vi.advanceTimersByTimeAsync(POLL);
	const last = asked;
	await vi.advanceTimersByTimeAsync(3 * POLL);
	expect(asked).toBe(last);
	stop();
	client.clear();
});
