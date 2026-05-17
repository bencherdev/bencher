import type { Params } from "astro";
import { createEffect, createMemo, createResource } from "solid-js";
import {
	activeAlertCount,
	fetchActiveAlertCount,
} from "../../../util/active_alerts";
import { authUser } from "../../../util/auth";
import { init_valid, validJwt } from "../../../util/valid";
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

	createEffect(() => {
		const valid = bencher_valid();
		const slug = params()?.project;
		const token = user?.token;
		if (!valid || !slug || !token || !validJwt(token)) {
			return;
		}
		fetchActiveAlertCount(props.apiUrl, slug, token);
	});

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
