import "@bencherdev/ui/styles.css";
import "../reports/reports.css";
import { render } from "solid-js/web";
import { expect, test } from "vitest";
import RunsTable from "./RunsTable";
import { runFixture } from "./testing";

/** Whether the page has fetched the code that draws a run's alerts, which loads once per page. */
const alertsCodeLoaded = () =>
	performance
		.getEntriesByType("resource")
		.some((entry) => entry.name.includes("/runs/RunAlerts.tsx"));

const move = (link: Element, movementX: number) =>
	link.dispatchEvent(
		new PointerEvent("pointermove", { bubbles: true, movementX }),
	);

// Kills a row that loads the alerts' code for a run that raised none, as the
// page draws it under a still pointer, or not until the run's page needs it.
test("moving onto a run that raised alerts starts their code", async () => {
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	render(
		() => (
			<RunsTable
				slug="hashbrown"
				runs={[
					runFixture(0),
					runFixture(1, { alerts: { total: 2, active: 1 } }),
				]}
				total={2}
				now={Date.parse("2024-03-01T00:00:00Z")}
			/>
		),
		root,
	);
	const [quiet, alerting] = [
		...document.querySelectorAll("tbody tr[aria-rowindex] a.rowlink-name"),
	];
	if (!(quiet && alerting)) {
		throw new Error("Both runs draw their links");
	}
	move(quiet, 4);
	move(alerting, 0);
	await new Promise((resolve) => setTimeout(resolve, 200));
	expect(alertsCodeLoaded()).toBe(false);

	move(alerting, 4);
	await expect.poll(alertsCodeLoaded).toBe(true);
});
