import "./page.css";
import { afterEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import { perf } from "../mcp/testing";
import { boot } from "./main";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;
// Every box filled, so the embed asks for the plot as it boots.
const DRAWABLE = `?branches=${uuid(1)}&testbeds=${uuid(3)}&benchmarks=${uuid(5)}&measures=${uuid(7)}`;
const home = `${location.pathname}${location.search}`;

afterEach(() => {
	vi.unstubAllGlobals();
	history.replaceState(null, "", home);
	document.body.replaceChildren();
});

// Kills an embed frame that never opens a pinned point's commit, opens it
// from anything but the project's URL, or asks its reader to link GitHub.
test("the embed opens a pinned point's commit through the project's URL", async () => {
	const hash = `9c1f2e4${"0".repeat(33)}`;
	const { project, url: _url, ...answer } = perf();
	vi.stubGlobal(
		"fetch",
		vi.fn(async (input: string) =>
			Response.json(
				new URL(input, location.href).pathname.endsWith("/perf")
					? {
							...answer,
							reports: answer.reports.map((report) => ({ ...report, hash })),
						}
					: {
							...project,
							visibility: "public",
							url: "https://github.com/pompeii-llc/hashbrown",
							github_link: "unlinked",
						},
			),
		),
	);
	history.replaceState(null, "", `${location.pathname}${DRAWABLE}`);
	const mount = document.createElement("div");
	mount.dataset.slug = "hashbrown";
	document.body.append(mount);
	boot(mount);

	await expect
		.element(page.getByRole("link", { name: "Hashbrown", exact: true }))
		.toBeVisible();
	const area = page.getByRole("group", { name: /lines?,/ });
	await expect.element(area).toBeVisible();
	const box = area.element().getBoundingClientRect();
	await area.click({ position: { x: box.width / 2, y: box.height / 2 } });
	const status = page.getByRole("region", { name: "Plot" }).getByRole("status");
	await expect
		.element(status.getByRole("link", { name: "Open commit on GitHub" }))
		.toHaveAttribute(
			"href",
			`https://github.com/pompeii-llc/hashbrown/commit/${hash}`,
		);
	expect(status.element().textContent).not.toContain("GitHub");
});
