import type { APIRequestContext } from "@playwright/test";
import { decodeQuery } from "../src/next/query/query";
import {
	type SelectedLine,
	exploreSearch,
	keptLines,
} from "../src/next/query/selection";
import { perfSearch } from "../src/next/explore/request";
import type { JsonConsolePerf } from "../src/types/bencher";
import { dimensions } from "./explore";
import { consolePath, expect, seed, signedIn, test } from "./fixtures";

const { hashbrown } = seed.projects;

test.use({ storageState: signedIn(seed.member), timezoneId: "UTC" });

const read = async (request: APIRequestContext, search: string) => {
	const response = await request.get(
		`${seed.api_url}/v0/console/projects/${hashbrown.slug}/perf${search}`,
		{ headers: { Authorization: `Bearer ${seed.member.token}` } },
	);
	expect(response.status()).toBe(200);
	return (await response.json()) as JsonConsolePerf;
};

const linesOf = (perf: JsonConsolePerf): SelectedLine[] =>
	perf.lines.map((line) => {
		const variant = perf.variants[line.variant];
		return {
			branch: { uuid: perf.branches[line.branch]?.uuid ?? "" },
			testbed: { uuid: perf.testbeds[line.testbed]?.uuid ?? "" },
			benchmark: perf.benchmarks[line.benchmark]?.uuid ?? "",
			parameters: variant?.parameters ?? {},
			measure: perf.measures[line.measure]?.uuid ?? "",
			metric: line.metric,
			alerting: false,
		};
	});

const id = (line: SelectedLine) =>
	JSON.stringify([
		line.branch.uuid,
		line.testbed.uuid,
		line.benchmark,
		line.parameters,
		line.measure,
		line.metric,
	]);

// Kills a selection the perf API cuts short with no word of the lines it
// left out, which the Open N link counted.
test("a selection past the perf API's cut says how many of its lines it drew", async ({
	page,
	request,
}) => {
	const ids = await dimensions(request, hashbrown.slug);
	const benchmarks = ["blake3", "sha256", "xxh3", "crc32c"].map(ids.benchmark);
	const branches = [ids.branch("main"), ids.branch("feature-simd")];
	const measures = [ids.measure("Latency"), ids.measure("Throughput")];
	const testbed = ids.testbed("ubuntu-latest");
	const window = { seconds: 365 * 86_400 };
	const last = await read(
		request,
		perfSearch(
			{
				...decodeQuery(""),
				branches: [{ uuid: branches[1] ?? "" }],
				testbeds: [{ uuid: testbed }],
				benchmarks: [benchmarks[3] ?? ""],
				measures: [measures[1] ?? ""],
				window,
			},
			Date.now(),
		),
	);
	const first = await read(
		request,
		perfSearch(
			{
				...decodeQuery(""),
				branches: [{ uuid: branches[0] ?? "" }],
				testbeds: [{ uuid: testbed }],
				benchmarks: benchmarks.slice(0, 3),
				measures: [measures[0] ?? ""],
				window,
			},
			Date.now(),
		),
	);
	const tail = linesOf(last).at(-1);
	if (!tail) {
		throw new Error("no last line");
	}
	const chosen = [...linesOf(first), tail];
	const kept = keptLines(chosen);
	const search = exploreSearch(chosen, { window });
	const query = decodeQuery(search);
	const answer = await read(request, perfSearch(query, Date.now()));
	const returned = new Set(linesOf(answer).map(id));
	const missing = kept.filter((line) => !returned.has(id(line)));
	await page.goto(`${consolePath(hashbrown.slug, "explore")}${search}`);
	const cap = page.locator(".ex-cap");
	await expect(cap).toBeVisible();
	const capText = await cap.innerText();
	expect(capText).toContain(
		`${kept.length - missing.length} of ${kept.length} lines`,
	);
});

// Kills a note shown for a selection drawn whole.
test("a selection the perf API draws whole says nothing of its lines", async ({
	page,
	request,
}) => {
	const ids = await dimensions(request, hashbrown.slug);
	const window = { seconds: 365 * 86_400 };
	const first = await read(
		request,
		perfSearch(
			{
				...decodeQuery(""),
				branches: [{ uuid: ids.branch("main") }],
				testbeds: [{ uuid: ids.testbed("ubuntu-latest") }],
				benchmarks: [ids.benchmark("blake3"), ids.benchmark("sha256")],
				measures: [ids.measure("Latency")],
				window,
			},
			Date.now(),
		),
	);
	const chosen = linesOf(first).slice(0, 3);
	const search = exploreSearch(chosen, { window });
	await page.goto(`${consolePath(hashbrown.slug, "explore")}${search}`);
	await expect(page.getByRole("figure").first()).toHaveAccessibleName(
		`${chosen.length} lines`,
	);
	await expect(page.locator(".ex-cap")).toHaveCount(0);
});
