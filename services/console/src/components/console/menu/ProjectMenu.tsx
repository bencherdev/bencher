import type { Params } from "astro";
import { createMemo, createResource } from "solid-js";
import {
	activeAlertCount,
	fetchActiveAlertCount,
} from "../../../util/active_alerts";
import { authUser } from "../../../util/auth";
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
		project_slug: undefined | string;
		token: undefined | string;
	}) => {
		if (
			!fetcher.bencher_valid ||
			!fetcher.project_slug ||
			!fetcher.token ||
			!validJwt(fetcher.token)
		) {
			return;
		}
		return await fetchActiveAlertCount(
			props.apiUrl,
			fetcher.project_slug,
			fetcher.token,
		);
	};
	createResource(fetcher, getAlerts);

	const active_alerts = createMemo(() => {
		const slug = params()?.project;
		if (!slug) {
			return undefined;
		}
		return activeAlertCount(slug);
	});

	return <ProjectMenuInner project={project} active_alerts={active_alerts} />;
};

export default ProjectMenu;
