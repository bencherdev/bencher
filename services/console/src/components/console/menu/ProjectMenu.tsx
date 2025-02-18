import * as Sentry from "@sentry/astro";
import type { Params } from "astro";
import { createMemo, createResource } from "solid-js";
import { authUser } from "../../../util/auth";
import { X_TOTAL_COUNT, httpGet } from "../../../util/http";
import { type InitValid, init_valid, validJwt } from "../../../util/valid";
import ProjectMenuInner from "./ProjectMenuInner";

interface Props {
	apiUrl: string;
	params: Params;
}

const ProjectMenu = (props: Props) => {
	const [bencher_valid] = createResource(init_valid);
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
		bencher_valid: InitValid;
		project_slug: string;
		token: string;
	}) => {
		if (
			!fetcher.bencher_valid ||
			!fetcher.project_slug ||
			!validJwt(fetcher.token)
		) {
			return;
		}
		const pathname = `/v0/projects/${fetcher.project_slug}/alerts?per_page=0&status=active`;
		return await httpGet(props.apiUrl, pathname, authUser()?.token)
			.then((resp) => resp?.headers?.[X_TOTAL_COUNT])
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return;
			});
	};
	const [active_alerts] = createResource<undefined | number>(
		fetcher,
		getAlerts,
	);

	return <ProjectMenuInner project={project} active_alerts={active_alerts} />;
};

export default ProjectMenu;
