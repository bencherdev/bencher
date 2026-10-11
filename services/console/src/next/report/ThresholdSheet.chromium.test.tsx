import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "./report.css";
import { QueryClientProvider } from "@tanstack/solid-query";
import { render } from "solid-js/web";
import { afterEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import type { Api } from "../api";
import { createQueryClient } from "../cache";
import { ProjectContext } from "../project";
import { fakeApi } from "../reports/testing";
import { type ReportLine, linesOf } from "./lines";
import { REPORT, lineFixture, reportFixture } from "./testing";
import ThresholdSheet from "./ThresholdSheet";

const CLOUD = "https://api.bencher.dev";
const SELF_HOSTED = "http://localhost:61016";

const [latency, throughput] = linesOf(
	reportFixture([
		lineFixture(),
		lineFixture({
			measure: 1,
			model: undefined,
			baseline: undefined,
			upper_limit: undefined,
		}),
	]),
) as [ReportLine, ReportLine];

let dispose: (() => void) | undefined;

afterEach(() => {
	vi.restoreAllMocks();
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

/** The sheet for `line`, its counts answered by `answer`, as the API counts with and without the parameters filter. */
const mount = ({
	line = throughput,
	lines = [latency, throughput],
	host = CLOUD,
	api = countingApi().api,
}: {
	line?: ReportLine;
	lines?: ReportLine[];
	host?: string;
	api?: Api;
} = {}) => {
	const client = createQueryClient(() => {});
	client.setDefaultOptions({
		queries: { ...client.getDefaultOptions().queries, retry: false },
	});
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<QueryClientProvider client={client}>
				<ProjectContext.Provider value={{ api, slug: () => "hashbrown" }}>
					<ThresholdSheet
						line={line}
						lines={lines}
						report={REPORT}
						branch="main"
						testbed="ubuntu-latest"
						host={host}
						onClose={() => {}}
					/>
				</ProjectContext.Provider>
			</QueryClientProvider>
		),
		root,
	);
};

/** Two lines carry this line's parameters; eighteen share its measure and metric. */
const countingApi = (fail = () => false) =>
	fakeApi((url) =>
		fail()
			? Promise.reject(new Error("down"))
			: {
					data: {
						...reportFixture([]),
						total: url.searchParams.has("parameters") ? 2 : 18,
					},
				},
	);

const exact = () => page.getByRole("region", { name: "Exactly this line" });
const simple = () =>
	page.getByRole("region", { name: "Every line of the measure" });

// Kills counts swapped between the two thresholds.
test("each threshold says how many of the report's lines it checks", async () => {
	mount();
	await expect
		.element(exact())
		.toHaveTextContent("2 lines in this report, this one included");
	await expect
		.element(simple())
		.toHaveTextContent("18 lines in this report, this one included");
});

// Kills a self-hosted server left out of the commands, and Bencher Cloud named in them.
test("a self-hosted API is named in both commands, and Bencher Cloud in neither", async () => {
	mount({ host: SELF_HOSTED });
	await expect.element(exact()).toHaveTextContent(`--host ${SELF_HOSTED}`);
	await expect.element(simple()).toHaveTextContent(`--host ${SELF_HOSTED}`);

	dispose?.();
	document.body.replaceChildren();
	mount();
	await expect.element(exact()).not.toHaveTextContent("--host");
	await expect.element(simple()).not.toHaveTextContent("--host");
});

// Kills a Copy that copies nothing, or one command in place of the other.
test("each Copy puts its own command on the clipboard", async () => {
	const writeText = vi
		.spyOn(navigator.clipboard, "writeText")
		.mockResolvedValue(undefined);
	mount();
	await exact()
		.getByRole("button", { name: "Copy the command for exactly this line" })
		.click();
	await simple()
		.getByRole("button", {
			name: "Copy the command for every line of the measure",
		})
		.click();
	const [[exactText], [simpleText]] = writeText.mock.calls as [
		[string],
		[string],
	];
	expect(exactText).toBe(
		[
			"export BENCHER_API_KEY=bencher_run_...",
			"bencher run \\",
			"  --project hashbrown \\",
			"  --branch main \\",
			"  --testbed ubuntu-latest \\",
			"  --adapter json \\",
			"  --threshold-measure throughput \\",
			"  --threshold-metric value \\",
			`  --threshold-parameters '{"input_bytes": 65536, "simd": "avx2", "threads": 1}' \\`,
			"  --threshold-test t_test \\",
			"  --threshold-max-sample-size 30 \\",
			"  --threshold-lower-boundary 0.99 \\",
			"  --file results.json",
		].join("\n"),
	);
	expect(simpleText).toBe(
		exactText.replace(/\n {2}--threshold-parameters [^\n]*/, ""),
	);
	await expect
		.element(simple().getByRole("button", { name: /^Copy / }))
		.toHaveTextContent("Copied");
});

// Kills a refused copy that fails without a word.
test("a copy the browser refuses says so", async () => {
	vi.spyOn(navigator.clipboard, "writeText").mockRejectedValue(
		new DOMException("Write permission denied.", "NotAllowedError"),
	);
	mount();
	await simple()
		.getByRole("button", { name: /^Copy / })
		.click();
	await expect
		.element(simple().getByRole("alert"))
		.toHaveTextContent(
			"The browser refused to copy; select the command to copy it.",
		);
});

// Kills a count that failed and holds its placeholder for good.
test("a count that did not load offers Retry, which loads it", async () => {
	let down = true;
	const { api } = countingApi(() => down);
	mount({ api });
	await expect.element(simple()).toHaveTextContent("the count did not load");
	down = false;
	await simple().getByRole("button", { name: "Retry" }).click();
	await expect
		.element(simple())
		.toHaveTextContent("18 lines in this report, this one included");
});

// Kills a threshold that guards against the default side when the report's other lines show the project's.
test("a threshold guards the side the project already guards for that measure", async () => {
	const [falling, unguarded] = linesOf(
		reportFixture(
			[
				lineFixture({ model: 1, upper_limit: undefined, lower_limit: 19 }),
				lineFixture({
					variant: 1,
					model: undefined,
					baseline: undefined,
					upper_limit: undefined,
				}),
			],
			{
				models: [
					{
						uuid: "upper",
						threshold: "upper",
						test: "t_test",
						upper_boundary: 0.99,
					},
					{
						uuid: "lower",
						threshold: "lower",
						test: "t_test",
						lower_boundary: 0.99,
					},
				],
			} as never,
		),
	) as [ReportLine, ReportLine];
	mount({ line: unguarded, lines: [falling, unguarded] });
	await expect
		.element(exact())
		.toHaveTextContent("--threshold-lower-boundary 0.99");
	await expect.element(exact()).not.toHaveTextContent("--threshold-upper");
});
