import { expect, test } from "vitest";
import type { JsonUsage } from "../../types/bencher";
import type { Api } from "../api";
import { contractQuery, endsAContract } from "./contract";

const usage = (level: string, status: string) =>
	({ plan: { level, status } }) as unknown as JsonUsage;

// Kills a delete offered while an Enterprise plan is live in any status, or
// held back for a Pro plan, an Enterprise plan that has ended, or no plan.
test("only a live Enterprise plan ends a contract", () => {
	for (const status of [
		"active",
		"trialing",
		"past_due",
		"unpaid",
		"incomplete",
		"paused",
	]) {
		expect(endsAContract(usage("enterprise", status)), status).toBe(true);
		expect(endsAContract(usage("pro", status)), status).toBe(false);
	}
	for (const status of ["canceled", "incomplete_expired"]) {
		expect(endsAContract(usage("enterprise", status)), status).toBe(false);
	}
	expect(endsAContract({} as JsonUsage)).toBe(false);
});

// Kills a plan read sent to another organization or endpoint, an answer that
// is not the contract's, and the plan or its payer kept in the cache the
// browser stores.
test("the contract read asks for the organization's plan and keeps only the answer", async () => {
	const reads: string[] = [];
	const api = {
		get: async (path: string) => {
			reads.push(path);
			return {
				data: {
					kind: "cloud_metered",
					plan: {
						level: "enterprise",
						status: "active",
						customer: { name: "Muriel Bagge", email: "m@nowhere.example" },
					},
				},
				headers: new Headers(),
			};
		},
	} as unknown as Api;
	const query = contractQuery(api, "o1");
	expect(query.queryKey).toEqual(["organization", "o1", "contract"]);
	await expect(
		query.queryFn({ signal: new AbortController().signal }),
	).resolves.toBe(true);
	expect(reads).toEqual(["/v0/organizations/o1/usage"]);
});
