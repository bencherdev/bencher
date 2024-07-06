import type { Params } from "astro";
import bencher_valid_init, { type InitOutput } from "bencher_valid";
import { createMemo, createResource } from "solid-js";
import type { JsonAlertStats } from "../../../types/bencher";
import { authUser } from "../../../util/auth";
import { httpGet } from "../../../util/http";
import { validJwt } from "../../../util/valid";
import ProjectMenuInner from "./ProjectMenuInner";
import * as Sentry from "@sentry/astro";

interface Props {
	apiUrl: string;
	params: Params;
}

const ProjectMenu = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const params = createMemo(() => props.params);
	const project = createMemo(() => params().project);
	const user = authUser();

	const fetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			project_slug: params()?.project,
			token: user?.token,
		};
	});
	const getAlerts = async (fetcher: {
		bencher_valid: InitOutput;
		project_slug: string;
		token: string;
	}): Promise<JsonAlertStats> => {
		const DEFAULT_ALERT_STATS = {
			active: 0,
		};
		if (
			!fetcher.bencher_valid ||
			!fetcher.project_slug ||
			!validJwt(fetcher.token)
		) {
			return DEFAULT_ALERT_STATS;
		}
		const pathname = `/v0/projects/${fetcher.project_slug}/stats/alerts`;
		return await httpGet(props.apiUrl, pathname, authUser()?.token)
			.then((resp) => resp.data)
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return DEFAULT_ALERT_STATS;
			});
	};
	const [alert_stats] = createResource<JsonAlertStats>(fetcher, getAlerts);
	const active_alerts = createMemo(() => alert_stats()?.active);

	return <ProjectMenuInner project={project} active_alerts={active_alerts} />;
};

export default ProjectMenu;
