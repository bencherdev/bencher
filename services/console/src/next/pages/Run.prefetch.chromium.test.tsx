import { expect, test } from "vitest";
import { alertsFixture } from "../alerts/testing";
import { createQueryClient } from "../cache";
import { fakeApi } from "../reports/testing";
import { runPageFixture, runReportFixture } from "../runs/testing";
import { prefetch } from "./Run";

/** Whether the page has fetched the code that draws a run's alerts, which loads once per page. */
const alertsCodeLoaded = () =>
	performance
		.getEntriesByType("resource")
		.some((entry) => entry.name.includes("/runs/RunAlerts.tsx"));

// Kills a prefetch that loads the alerts' code for a run that raised none, or
// leaves it for the page to find after the move for a run that raised some.
test("prefetching a run starts its alerts' code only when it raised any", async () => {
	const client = createQueryClient(() => {});
	const quiet = fakeApi(() => ({
		data: runPageFixture([runReportFixture(0)]),
	}));
	await prefetch(client, quiet.api, "hashbrown", "quiet");
	await new Promise((resolve) => setTimeout(resolve, 200));
	expect(quiet.requests).toHaveLength(1);
	expect(alertsCodeLoaded()).toBe(false);

	const alerting = fakeApi(() => ({
		data: runPageFixture([runReportFixture(0)], {
			alerts: { total: 1, active: 1 },
			alerting: alertsFixture([
				{ report: 0, order: 1, alerts: [{ alert: 1 }] },
			]),
		}),
	}));
	await prefetch(client, alerting.api, "hashbrown", "alerting");
	await expect.poll(alertsCodeLoaded).toBe(true);
});
