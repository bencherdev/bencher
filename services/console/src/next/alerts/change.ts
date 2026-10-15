import { useQueryClient } from "@tanstack/solid-query";
import { createSignal } from "solid-js";
import { alertPath } from "../paths";
import type { PlotAlerts } from "../plot/Plot";
import { useProject } from "../project";
import type { Settable } from "./status";

/**
 * Dismiss and Reactivate on one alert at a time, outside a list of its rows:
 * shown at once with the Alerts count, and put back if the API refuses or
 * changes nothing. The status a page shows is `status` over its data's; a page
 * whose plots come and go keeps them in `kept`.
 */
export const useAlertChange = (kept = new Map<string, Settable>()) => {
	const project = useProject();
	const client = useQueryClient();
	const [statuses, setStatuses] = createSignal(kept, { equals: false });
	/** The alert whose last change did not go through. */
	const [refused, setRefused] = createSignal<string>();
	const status = (uuid: string) => statuses().get(uuid);
	// Each alert's newest change, so an older one that fails puts back nothing a newer one set.
	const newest = new Map<string, number>();
	let requests = 0;
	const change = (uuid: string, next: Settable) => {
		const request = ++requests;
		newest.set(uuid, request);
		const set = (value: Settable | undefined) => {
			if (value) {
				kept.set(uuid, value);
			} else {
				kept.delete(uuid);
			}
			setStatuses(kept);
		};
		const before = status(uuid);
		const undo = () => {
			const latest = newest.get(uuid) === request;
			if (latest) {
				set(before);
				setRefused(uuid);
			}
			return latest;
		};
		set(next);
		setRefused();
		// The change's own code waits for its first use.
		import("./write").then(
			({ changeAlert }) => changeAlert(project, client, uuid, next, undo),
			undo,
		);
	};
	const isRefused = (uuid: string) => refused() === uuid;
	return {
		/** The status this page set on the alert, if any. */
		status,
		change,
		refused: isRefused,
		/** What a pinned readout offers on an alert: its page, and while `canChange`, Dismiss or Reactivate here. */
		readout: (canChange: () => boolean): PlotAlerts => ({
			href: (uuid) => alertPath(project.slug(), uuid),
			status,
			change: (uuid) =>
				canChange() ? (next) => change(uuid, next) : undefined,
			refused: isRefused,
		}),
	};
};
