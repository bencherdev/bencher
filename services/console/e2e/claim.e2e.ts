import { randomUUID } from "node:crypto";
import type { JsonReport } from "../src/types/bencher";
import { createReader } from "./api";
import { axeInBothThemes } from "./axe";
import { consolePath, expect, seed, signedIn, test } from "./fixtures";

// Kills a claim made as the page loads, with no press, one that names
// another organization, one that leaves a signed-in reader anywhere but the
// project its run made, one that gets there by way of the console's root, and
// one that claims nothing.
test("a signed-in reader who opens a claim link presses Claim and lands on the claimed project's Runs", async ({
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
	const reader = await createReader();
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
	const claim = page.getByRole("heading", {
		level: 1,
		name: `Claim ${project.name}`,
	});
	await expect(claim).toBeVisible();
	const unclaimed = await request.get(
		`${seed.api_url}/v0/organizations/${project.organization}`,
		{ headers: { Authorization: `Bearer ${reader.key}` } },
	);
	expect(unclaimed.status()).toBe(404);

	await page.getByRole("button", { name: "Claim", exact: true }).click();
	await expect(page).toHaveURL(consolePath(project.slug));
	expect(documents).toEqual(["/auth/signup", consolePath(project.slug)]);
	await expect(
		page.getByRole("table", { name: "Runs, newest first" }),
	).toBeVisible();
	// Only a member reads an organization.
	const claimed = await request.get(
		`${seed.api_url}/v0/organizations/${project.organization}`,
		{ headers: { Authorization: `Bearer ${reader.key}` } },
	);
	expect(claimed.status()).toBe(200);
});

// Kills a claim press drawn for an organization someone else claimed, which
// would only be refused, and a press that fails axe in either theme.
test("a claimed organization says so, and the press passes axe in both themes", async ({
	newContext,
	request,
}) => {
	const start = new Date(Date.parse(seed.now) - 60 * 60 * 1000);
	const run = await request.post(`${seed.api_url}/v0/run`, {
		data: {
			context: {
				"bencher.dev/v0/repo/name": `pressed-${randomUUID().slice(0, 8)}`,
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
	const reader = await createReader();
	const page = await (
		await newContext({ storageState: signedIn(reader) })
	).newPage();
	const path = `/auth/signup?${new URLSearchParams({ claim: project.organization })}`;
	await axeInBothThemes(page, path, () =>
		expect(
			page.getByRole("heading", { level: 1, name: `Claim ${project.name}` }),
		).toBeVisible(),
	);

	const claimer = await createReader();
	const claimed = await request.post(
		`${seed.api_url}/v0/organizations/${project.organization}/claim`,
		{ headers: { Authorization: `Bearer ${claimer.key}` }, data: {} },
	);
	expect(claimed.status()).toBe(201);
	await page.goto(path);
	await expect(
		page.getByRole("heading", {
			level: 1,
			name: "This organization is already claimed",
		}),
	).toBeVisible();
	await expect(
		page.getByRole("link", { name: "Back to the console" }),
	).toHaveAttribute("href", "/console");
});
