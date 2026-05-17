import * as Sentry from "@sentry/astro";
import { createRoot, createSignal } from "solid-js";
import { X_TOTAL_COUNT, httpGet } from "./http";

const BENCHER_ACTIVE_ALERTS_KEY = "BENCHER_ACTIVE_ALERTS";

interface ActiveAlerts {
	project: string;
	count: number;
}

const getStored = (): ActiveAlerts | null => {
	if (typeof localStorage === "undefined") {
		return null;
	}
	const stored = localStorage.getItem(BENCHER_ACTIVE_ALERTS_KEY);
	if (!stored) {
		return null;
	}
	return JSON.parse(stored);
};

const setStored = (value: ActiveAlerts) => {
	if (typeof localStorage !== "undefined") {
		localStorage.setItem(BENCHER_ACTIVE_ALERTS_KEY, JSON.stringify(value));
	}
};

const [alerts, setAlerts] = createRoot(() => {
	return createSignal<ActiveAlerts | null>(getStored());
});

export const activeAlertCount = (project: string): number | undefined => {
	const current = alerts();
	if (current && current.project === project) {
		return current.count;
	}
	return undefined;
};

export const fetchActiveAlertCount = async (
	apiUrl: string,
	project: string,
	token: string,
): Promise<number> => {
	const path = `/v0/projects/${project}/alerts?per_page=0&status=active`;
	return await httpGet(apiUrl, path, token)
		.then((resp) => {
			const count = Number.parseInt(resp?.headers?.[X_TOTAL_COUNT] ?? "0");
			const value = { project, count };
			setAlerts(value);
			setStored(value);
			return count;
		})
		.catch((error) => {
			console.error(error);
			Sentry.captureException(error);
			return activeAlertCount(project) ?? 0;
		});
};

export const invalidateActiveAlertCount = (project: string) => {
	const value = { project, count: 0 };
	setAlerts(value);
	setStored(value);
};
