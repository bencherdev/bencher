import type { CallToolResult } from "@modelcontextprotocol/client";
import { AlertStatus, BoundaryLimit, Visibility } from "../../types/bencher";
import type { PerfResult } from "./main";

export const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

export const CONSOLE = "https://bencher.test";
export const URL_ = `${CONSOLE}/console/projects/hashbrown/explore?branches=${uuid(1)}&testbeds=${uuid(3)}&benchmarks=${uuid(5)}&measures=${uuid(7)}`;
export const INPUT = {
	project: "hashbrown",
	branches: uuid(1),
	testbeds: uuid(3),
	benchmarks: uuid(5),
	measures: uuid(7),
};

/** `query_perf`'s result: two lines over three reports, the second line alerted on the middle one. */
export const perf = (name = "Hashbrown"): PerfResult => ({
	window: { start_time: 0, end_time: 3, clamped: false },
	total: 2,
	points: { x: [1, 2, 3], report: [0, 1, 2] },
	reports: [20, 21, 22].map((n) => ({ uuid: uuid(n), version: n })),
	branches: [{ uuid: uuid(1), name: "main", slug: "main", head: uuid(11) }],
	testbeds: [{ uuid: uuid(3), name: "linux", slug: "linux" }],
	benchmarks: [{ uuid: uuid(5), name: "blake3", slug: "blake3" }],
	variants: [
		{ uuid: uuid(30), benchmark: 0, parameters: { threads: 1 } },
		{ uuid: uuid(31), benchmark: 0, parameters: { threads: 4 } },
	],
	measures: [{ uuid: uuid(7), name: "Latency", slug: "latency", units: "ns" }],
	models: [],
	lines: [0, 1].map((variant) => ({
		branch: 0,
		testbed: 0,
		benchmark: 0,
		variant,
		measure: 0,
		metric: "value",
		series: {
			y: variant === 0 ? [1, 1, 1] : [2, 6, 2],
			upper: [4, 4, 4],
			alerts:
				variant === 0
					? []
					: [
							{
								index: 1,
								uuid: uuid(40),
								limit: BoundaryLimit.Upper,
								status: AlertStatus.Active,
							},
						],
		},
	})),
	project: {
		uuid: uuid(50),
		organization: uuid(51),
		name,
		slug: "hashbrown",
		visibility: Visibility.Private,
		bmf_version: 1,
		created: "2026-10-01T00:00:00Z",
		modified: "2026-10-01T00:00:00Z",
	},
	url: URL_,
});

export const success = (result: PerfResult): CallToolResult => ({
	content: [{ type: "text", text: JSON.stringify(result) }],
	structuredContent: result,
});

export const toolError = (text: string): CallToolResult => ({
	content: [{ type: "text", text }],
	isError: true,
});

/**
 * A host that draws the page for one `query_perf` call, as the MCP Apps
 * messages run: it answers the handshake in `theme`, sends the call's input and
 * `first`, answers each refresh with the next of `refreshes` (a JSON-RPC error
 * when it is `null`), opens each link, and keeps each size the page reports.
 * It sends to the page with `send` and hears the page through `answer`.
 */
export const fakeHost = (
	options: {
		theme?: "light" | "dark";
		first: CallToolResult;
		refreshes?: (CallToolResult | null)[];
	},
	send: (message: Record<string, unknown>) => void,
) => {
	const { theme, first, refreshes = [] } = options;
	const calls: Record<string, unknown>[] = [];
	const opened: string[] = [];
	const sizes: Record<string, unknown>[] = [];
	const post = (message: Record<string, unknown>) =>
		send({ jsonrpc: "2.0", ...message });
	const answer = ({ id, method, params = {} }: Sent) => {
		if (method === "ui/initialize") {
			post({
				id,
				result: {
					protocolVersion: params.protocolVersion,
					hostInfo: { name: "test-host", version: "0.0.0" },
					hostCapabilities: { openLinks: {}, serverTools: {} },
					hostContext: { displayMode: "inline", ...(theme && { theme }) },
				},
			});
		} else if (method === "ui/notifications/initialized") {
			post({
				method: "ui/notifications/tool-input",
				params: { arguments: INPUT },
			});
			post({ method: "ui/notifications/tool-result", params: first });
		} else if (method === "tools/call") {
			calls.push(params);
			const next = refreshes.shift();
			post(
				next
					? { id, result: next }
					: { id, error: { code: -32000, message: "Refused" } },
			);
		} else if (method === "ui/open-link") {
			opened.push(String(params.url));
			post({ id, result: {} });
		} else if (method === "ui/notifications/size-changed") {
			sizes.push(params);
		}
	};
	const notify = (method: string, params: Record<string, unknown>) =>
		post({ method, params });
	return { answer, calls, opened, sizes, notify };
};

interface Sent {
	id?: number;
	method?: string;
	params?: Record<string, unknown>;
}
