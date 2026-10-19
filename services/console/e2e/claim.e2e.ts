import { randomUUID } from "node:crypto";
import type { JsonReport } from "../src/types/bencher";
import { createReader } from "./api";
import { consolePath, expect, seed, signedIn, test } from "./fixtures";

// Kills a claim that leaves a signed-in reader anywhere but the project its
// run made, one that gets there by way of the console's root, and one that
// claims nothing.
test("a signed-in reader who opens a claim link lands on the claimed project's Runs", async ({
	newContext,
	request,
}) => {
	const start = new Date(Date.parse(seed.now) - 60 * 60 * 1000);
	// A run with no key makes a project in an unclaimed organization.
	const run = await request.post(`${seed.api_url}/v0/run`, {
		data: {
			context: {
				"bencher.dev/v0/repo/name": `claimed-${randomUUID().slice(0, 8)}`,
			},
			branch: "main",
			testbed: "ubuntu-latest",
			start_time: start.toISOString(),
			end_time: new Date(start.getTime() + 60 * 1000).toISOString(),
			results: [JSON.stringify({ hash: { latency: { value: 1 } } })],
			settings: { adapter: "json" },
		},
	});
	expect(run.status(), await run.text()).toBe(201);
	const { project } = (await run.json()) as JsonReport;
	const reader = await createReader(request);
	const page = await (
		await newContext({ storageState: signedIn(reader) })
	).newPage();

	const documents: string[] = [];
	page.on("request", (sent) => {
		if (sent.resourceType() === "document") {
			documents.push(new URL(sent.url()).pathname);
		}
	});
	await page.goto(
		`/auth/signup?${new URLSearchParams({ claim: project.organization })}`,
	);

	await expect(page).toHaveURL(consolePath(project.slug));
	expect(documents).toEqual(["/auth/signup", consolePath(project.slug)]);
	await expect(
		page.getByRole("table", { name: "Runs, newest first" }),
	).toBeVisible();
	// Only a member reads an organization.
	const claimed = await request.get(
		`${seed.api_url}/v0/organizations/${project.organization}`,
		{ headers: { Authorization: `Bearer ${reader.token}` } },
	);
	expect(claimed.status()).toBe(200);
});
