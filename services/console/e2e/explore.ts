import type { APIRequestContext } from "@playwright/test";
import {
	type ExploreQuery,
	blankQuery,
	encodeQuery,
} from "../src/next/query/query";
import { nextPath, seed } from "./fixtures";

const { hashbrown } = seed.projects;

type Dimension = "branches" | "testbeds" | "benchmarks" | "measures";

/** Each dimension's UUIDs by name, as the API lists them. */
export const dimensions = async (
	request: APIRequestContext,
	project: string,
) => {
	const read = async (dimension: Dimension) => {
		const response = await request.get(
			`${seed.api_url}/v0/projects/${project}/${dimension}?per_page=64`,
			{ headers: { Authorization: `Bearer ${seed.member.token}` } },
		);
		const list = (await response.json()) as { uuid: string; name: string }[];
		return (name: string) => {
			const found = list.find((entry) => entry.name === name);
			if (!found) {
				throw new Error(`No ${dimension} named ${name}`);
			}
			return found.uuid;
		};
	};
	return {
		branch: await read("branches"),
		testbed: await read("testbeds"),
		benchmark: await read("benchmarks"),
		measure: await read("measures"),
	};
};

export const explore = (project: string, query?: Partial<ExploreQuery>) =>
	`${nextPath(project)}${query ? encodeQuery({ ...blankQuery(), ...query }) : ""}`;

/** The seed's alerting variant and the one beside it: blake3 at 64 KiB on one thread. */
export const twoLines = async (request: APIRequestContext) => {
	const id = await dimensions(request, hashbrown.slug);
	return {
		id,
		query: {
			branches: [{ uuid: id.branch("main") }],
			testbeds: [{ uuid: id.testbed("ubuntu-latest") }],
			benchmarks: [id.benchmark("blake3")],
			sets: [{ input_bytes: 65536, threads: 1 }],
			measures: [id.measure("Latency")],
			metrics: ["value"],
		},
	};
};
