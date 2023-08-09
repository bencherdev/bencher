import bencher_valid_init from "bencher_valid";
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

const BENCHER_ALL_PROJECTS = "--bencher--all---projects--";

interface Props {
	params: Params;
	user: JsonAuthUser;
}

const ProjectSelect = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);

	const navigate = useNavigate();
	const isValidJwt = createMemo(() =>
		bencher_valid() ? validJwt(props.user.token) : false,
	);

	const url = createMemo(
		() =>
			`${BENCHER_API_URL()}/v0/organizations/${
				props.params.organization
			}/projects`,
	);

	const fetchProjects = async () => {
		const ALL_PROJECTS = {
			name: "All Projects",
			slug: BENCHER_ALL_PROJECTS,
			uuid: "",
			organization: "",
			visibility: JsonVisibility.Public,
			created: "",
			modified: "",
		};
		const token = props.user.token;
		if (!isValidJwt()) {
			return [ALL_PROJECTS];
		}
		return await httpGet(url(), token)
			.then((resp) => {
				let data: JsonProject[] = resp?.data;
				data.push(ALL_PROJECTS);
				return data;
			})
			.catch((error) => {
				console.error(error);
				return [ALL_PROJECTS];
			});
	};

	const getSelected = () => {
		const slug = props.params.project;
		if (slug === null) {
			return BENCHER_ALL_PROJECTS;
		} else {
			return slug;
		}
	};

	const [selected, setSelected] = createSignal(getSelected());
	const [projects] = createResource(selected, fetchProjects);

	createEffect(() => {
		setSelected(getSelected());
	});

	const handleSelectedRedirect = () => {
		let path: string;
		if (selected() === BENCHER_ALL_PROJECTS) {
			path = `/console/organizations/${props.params.organization}/projects`;
		} else {
			path = `/console/projects/${selected()}/perf`;
		}
		navigate(path);
	};

	const handleInput = (target: string) => {
		if (target === BENCHER_ALL_PROJECTS) {
			setSelected(target);
			navigate(`/console/organizations/${props.params.organization}/projects`);
			return;
		}

		const p = projects();
		for (let i in p) {
			const project = p[i];
			const slug = project?.slug;
			if (slug === target) {
				handleSelectedRedirect();
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
