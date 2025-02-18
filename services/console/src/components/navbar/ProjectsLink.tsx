import * as Sentry from "@sentry/astro";
import type { Params } from "astro";
import { createMemo, createResource } from "solid-js";
import type { JsonAuthUser, JsonProject } from "../../types/bencher";
import { httpGet } from "../../util/http";
import { type InitValid, init_valid, validJwt } from "../../util/valid";

interface Props {
	apiUrl: string;
	params: Params;
	user: JsonAuthUser;
}

const ProjectsLink = (props: Props) => {
	const [bencher_valid] = createResource(init_valid);
	const params = createMemo(() => props.params);

	const orgFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			project_slug: params()?.project,
			token: props.user?.token,
		};
	});
	const fetchOrg = async (fetcher: {
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
		const path = `/v0/projects/${fetcher.project_slug}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				const json_project: JsonProject = resp?.data;
				return json_project.organization;
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return;
			});
	};
	const [organization] = createResource<string>(orgFetcher, fetchOrg);

	return (
		<a
			class="navbar-item"
			href={`/console/organizations/${organization()}/projects`}
		>
			Projects
		</a>
	);
};

export default ProjectsLink;
