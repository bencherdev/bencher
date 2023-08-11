import bencher_valid_init, { type InitOutput } from "bencher_valid";
import { validJwt } from "../../../util/valid";
import {
	For,
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import { useNavigate } from "../../../util/url";
import { BENCHER_API_URL } from "../../../util/ext";
import { httpGet } from "../../../util/http";
import {
	JsonVisibility,
	type JsonProject,
	JsonAuthUser,
} from "../../../types/bencher";
import type { Params } from "astro";

const BENCHER_ALL_PROJECTS = "--bencher--all--projects--";

interface Props {
	params: Params;
	user: JsonAuthUser;
}

const ProjectSelect = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const params = createMemo(() => props.params);
	const navigate = useNavigate();

	const orgFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			organization_slug: params()?.organization,
			project_slug: params()?.project,
			token: props.user?.token,
		};
	});
	const fetchOrg = async (fetcher: {
		bencher_valid: InitOutput;
		organization_slug: string;
		project_slug: string;
		token: string;
	}) => {
		if (fetcher.organization_slug) {
			return fetcher.organization_slug;
		}
		if (!bencher_valid()) {
			return;
		}
		if (!validJwt(fetcher.token)) {
			return;
		}
		if (!fetcher.project_slug) {
			return;
		}
		const url = `${BENCHER_API_URL()}/v0/projects/${fetcher.project_slug}`;
		return await httpGet(url, fetcher.token)
			.then((resp) => {
				let json_project: JsonProject = resp?.data;
				return json_project.organization;
			})
			.catch((error) => {
				console.error(error);
				return;
			});
	};
	const [organization] = createResource(orgFetcher, fetchOrg);

	const getSelected = () => {
		const slug = params()?.project;
		if (slug === undefined) {
			return BENCHER_ALL_PROJECTS;
		} else {
			return slug;
		}
	};
	const [selected, setSelected] = createSignal(getSelected());

	const fetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			organization: organization(),
			project_slug: params()?.project,
			token: props.user?.token,
		};
	});
	const fetchProjects = async (fetcher: {
		bencher_valid: InitOutput;
		organization: string;
		project_slug: string;
		token: string;
	}) => {
		const ALL_PROJECTS = {
			name: "All Projects",
			slug: BENCHER_ALL_PROJECTS,
			uuid: "",
			organization: "",
			visibility: JsonVisibility.Public,
			created: "",
			modified: "",
		};
		if (!bencher_valid()) {
			return [ALL_PROJECTS];
		}
		if (!validJwt(fetcher.token)) {
			return [ALL_PROJECTS];
		}
		if (!fetcher.organization) {
			return [ALL_PROJECTS];
		}
		const url = `${BENCHER_API_URL()}/v0/organizations/${
			fetcher.organization
		}/projects?per_page=255`;
		return await httpGet(url, fetcher.token)
			.then((resp) => {
				const json_projects: JsonProject[] = resp?.data;
				json_projects.push(ALL_PROJECTS);
				return json_projects;
			})
			.catch((error) => {
				console.error(error);
				return [ALL_PROJECTS];
			});
	};
	const [projects] = createResource(fetcher, fetchProjects);

	createEffect(() => {
		setSelected(getSelected());
	});

	const handleInput = (target: string) => {
		if (target === BENCHER_ALL_PROJECTS) {
			navigate(`/console/organizations/${organization()}/projects`);
			return;
		}

		const p = projects();
		for (let i in p) {
			const project = p[i];
			const slug = project?.slug;
			if (slug === target) {
				navigate(`/console/projects/${slug}/perf`);
				break;
			}
		}
	};

	return (
		<div class="select">
			<select onInput={(e) => handleInput(e.currentTarget.value)}>
				<For each={projects()}>
					{(project) => (
						<option value={project.slug} selected={project.slug === selected()}>
							{project.name}
						</option>
					)}
				</For>
			</select>
		</div>
	);
};

export default ProjectSelect;
