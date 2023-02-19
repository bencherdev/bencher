import { useLocation, useNavigate, useParams } from "solid-app-router";
import { createEffect, createMemo, createResource, For } from "solid-js";
import { BENCHER_API_URL, get_options, validate_jwt } from "../site/util";
import ConsoleMenu from "./menu/ConsoleMenu";
import ConsolePanel from "./panel/ConsolePanel";
import axios from "axios";
import Notification from "../site/Notification";

export const organizationSlug = (pathname) => {
	const path = pathname().split("/");
	if (
		path.length < 5 ||
		path[0] ||
		path[1] !== "console" ||
		path[2] !== "organizations" ||
		!path[3] ||
		!path[4]
	) {
		return null;
	}
	return path[3];
};

export const projectSlug = (pathname) => {
	const path = pathname().split("/");
	if (
		path.length < 5 ||
		path[0] ||
		path[1] !== "console" ||
		path[2] !== "projects" ||
		!path[3]
	) {
		return null;
	}
	return path[3];
};

const ConsolePage = (props) => {
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);

	const params = useParams();
	const path_params = createMemo(() => params);

	const url = createMemo(
		() => `${BENCHER_API_URL()}/v0/projects/${props.project_slug()}`,
	);

	const fetchProject = async (project_slug: string) => {
		const EMPTY_OBJECT = {};
		try {
			const token = props.user?.token;
			if (!validate_jwt(props.user?.token)) {
				return EMPTY_OBJECT;
			}

			const resp = await axios(get_options(url(), token));
			return resp?.data;
		} catch (error) {
			console.error(error);
			return EMPTY_OBJECT;
		}
	};

	const [project] = createResource(props.project_slug, fetchProject);

	createEffect(() => {
		// if (!validate_jwt(props.user?.token)) {
		//   navigate("/auth/login");
		// }

		const organization_uuid = project()?.organization;
		if (organization_uuid) {
			props.handleOrganizationSlug(organization_uuid);
		} else {
			const slug = organizationSlug(pathname);
			props.handleOrganizationSlug(slug);
		}

		const project_slug = projectSlug(pathname);
		props.handleProjectSlug(project_slug);
	});

	return (
		<>
			<Notification />

			<section class="section">
				<div class="container">
					<div class="columns is-reverse-mobile">
						<div class="column is-narrow">
							<ConsoleMenu
								user={props.user}
								organization_slug={props.organization_slug}
								project_slug={props.project_slug}
								handleProjectSlug={props.handleProjectSlug}
							/>
						</div>
						<div class="column is-10">
							<ConsolePanel
								user={props.user}
								organization_slug={props.organization_slug}
								project_slug={props.project_slug}
								operation={props.operation}
								config={props.config}
								path_params={path_params}
								handleProjectSlug={props.handleProjectSlug}
							/>
							<For each={[...Array(3).keys()]}>{(_k, _i) => <br />}</For>
							<hr />
						</div>
					</div>
				</div>
			</section>
		</>
	);
};

export default ConsolePage;
