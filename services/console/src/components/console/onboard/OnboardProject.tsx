import bencher_valid_init, { type InitOutput, new_slug } from "bencher_valid";

import {
	Match,
	Switch,
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import { authUser } from "../../../util/auth";
import { useNavigate, useSearchParams } from "../../../util/url";
import {
	validJwt,
	validPlanLevel,
	validResourceName,
} from "../../../util/valid";
import { httpGet, httpPatch, httpPost } from "../../../util/http";
import type {
	JsonNewProject,
	JsonNewToken,
	JsonOrganization,
	JsonProject,
	JsonToken,
	PlanLevel,
} from "../../../types/bencher";
import Field, { type FieldHandler } from "../../field/Field";
import FieldKind from "../../field/kind";
import { set } from "mermaid/dist/diagrams/state/id-cache.js";
import { PLAN_PARAM, planParam } from "../../auth/auth";
import OnboardSteps, { OnboardStep } from "./OnboardSteps";

export interface Props {
	apiUrl: string;
}

const OnboardProject = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const user = authUser();
	const [searchParams, setSearchParams] = useSearchParams();
	const navigate = useNavigate();

	const plan = createMemo(() => searchParams[PLAN_PARAM] as PlanLevel);

	createEffect(() => {
		if (!bencher_valid()) {
			return;
		}

		const initParams: Record<string, null | string> = {};
		if (!validPlanLevel(searchParams[PLAN_PARAM])) {
			initParams[PLAN_PARAM] = null;
		}
		if (Object.keys(initParams).length !== 0) {
			setSearchParams(initParams);
		}
	});

	const orgsFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: user.token,
		};
	});
	const getOrganizations = async (fetcher: {
		bencher_valid: InitOutput;
		token: string;
	}) => {
		if (!fetcher.bencher_valid) {
			return undefined;
		}
		if (!validJwt(fetcher.token)) {
			return null;
		}
		const path = "/v0/organizations";
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				return null;
			});
	};
	const [organizations] = createResource<null | JsonOrganization[]>(
		orgsFetcher,
		getOrganizations,
	);

	const organization = createMemo(() =>
		Array.isArray(organizations()) && (organizations()?.length ?? 0) > 0
			? (organizations()?.[0] as JsonOrganization)
			: null,
	);

	const projectsFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: user.token,
			organization: organization(),
		};
	});
	const getProjects = async (fetcher: {
		bencher_valid: InitOutput;
		token: string;
		organization: undefined | JsonOrganization;
	}) => {
		if (!fetcher.bencher_valid) {
			return undefined;
		}
		if (!validJwt(fetcher.token)) {
			return null;
		}
		if (organizations.loading) {
			return undefined;
		}
		if (fetcher.organization === undefined) {
			return undefined;
		}
		if (fetcher.organization === null) {
			return null;
		}
		const path = `/v0/organizations/${fetcher.organization?.slug}/projects`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				return null;
			});
	};
	const [projects, { refetch: refetchProjects }] = createResource<
		null | JsonProject[]
	>(projectsFetcher, getProjects);

	const organizationProject = createMemo(() =>
		Array.isArray(projects()) && (projects()?.length ?? 0) > 0
			? (projects()?.[0] as JsonProject)
			: null,
	);

	const projectFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: user.token,
			organization: organization(),
			project: organizationProject(),
		};
	});
	const getProject = async (fetcher: {
		bencher_valid: InitOutput;
		token: string;
		organization: undefined | JsonOrganization;
		project: undefined | JsonProject;
	}) => {
		if (!fetcher.bencher_valid) {
			return undefined;
		}
		if (!validJwt(fetcher.token)) {
			return null;
		}
		if (organizations.loading || projects.loading) {
			return undefined;
		}
		if (fetcher.organization === undefined || fetcher.project === undefined) {
			return undefined;
		}
		if (fetcher.organization === null) {
			return null;
		}
		if (fetcher.project) {
			return fetcher.project;
		}
		const path = `/v0/organizations/${fetcher.organization?.slug}/projects`;
		const data: JsonNewProject = {
			name: `${user?.user?.name}'s project`,
		};
		return await httpPost(props.apiUrl, path, fetcher.token, data)
			.then((resp) => {
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				return null;
			});
	};
	const [project] = createResource<null | JsonProject>(
		projectFetcher,
		getProject,
	);

	const [renameProject, setRenameProject] = createSignal(null);
	const [renameValid, setRenameValid] = createSignal(null);
	const [submitting, setSubmitting] = createSignal(false);

	const handleField: FieldHandler = (_key, value, valid) => {
		setRenameProject(value);
		setRenameValid(valid);
	};

	const updateProjectFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: user.token,
			project: project(),
			renameProject: renameProject(),
			renameValid: renameValid(),
			submitting: submitting(),
		};
	});
	const updateProject = async (fetcher: {
		bencher_valid: InitOutput;
		token: string;
		project: undefined | JsonProject;
		renameProject: null | string;
		renameValid: null | boolean;
		submitting: boolean;
	}) => {
		if (!fetcher.submitting) {
			return null;
		}
		setSubmitting(false);
		if (!fetcher.bencher_valid) {
			return undefined;
		}
		if (!validJwt(fetcher.token)) {
			return null;
		}
		if (fetcher.project === undefined) {
			return undefined;
		}
		if (
			fetcher.project === null ||
			fetcher.renameProject === null ||
			fetcher.renameValid === null ||
			fetcher.renameValid === false
		) {
			return null;
		}
		const path = `/v0/projects/${fetcher.project?.slug}`;
		const data = {
			name: fetcher.renameProject,
			slug: new_slug(fetcher.renameProject),
		};
		return await httpPatch(props.apiUrl, path, fetcher.token, data)
			.then((resp) => {
				refetchProjects();
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				return null;
			});
	};
	const [_updatedProject] = createResource<null | JsonProject>(
		updateProjectFetcher,
		updateProject,
	);

	return (
		<>
			<OnboardSteps step={OnboardStep.PROJECT} plan={plan} />

			<section class="section">
				<div class="container">
					<div class="columns is-centered">
						<div class="column is-half">
							<div class="content has-text-centered">
								<h1 class="title is-1">Name your first project</h1>
								<h2 class="subtitle is-4">
									Pick a name for your project. You can always change it later.
								</h2>
								<form
									onSubmit={(e) => {
										e.preventDefault();
									}}
								>
									<Field
										kind={FieldKind.INPUT}
										fieldKey="name"
										value={renameProject() ?? project()?.name ?? ""}
										valid={renameValid()}
										config={{
											label: "Project Name",
											type: "text",
											placeholder: project()?.name ?? "Project Name",
											icon: "fas fa-project-diagram",
											help: "Must be non-empty string",
											validate: validResourceName,
										}}
										handleField={handleField}
									/>
									<button
										class="button is-primary is-outlined is-fullwidth"
										title="Save project name"
										onClick={(e) => {
											e.preventDefault;
											setSubmitting(true);
										}}
									>
										Save Project Name
									</button>
								</form>

								<figure class="frame">
									<h3 class="title is-5">Use this slug for your project</h3>
									<pre data-language="plaintext">
										<code>
											<div class="code">{project()?.slug}</div>
										</code>
									</pre>
									<button
										class="button is-outlined is-fullwidth"
										title="Copy project slug to clipboard"
										onClick={(e) => {
											e.preventDefault;
											navigator.clipboard.writeText(project()?.slug);
										}}
									>
										<span class="icon-text">
											<span class="icon">
												<i class="far fa-copy"></i>
											</span>
											<span>Copy to Clipboard</span>
										</span>
									</button>
								</figure>
								<br />
								<br />

								<a
									class="button is-primary is-fullwidth"
									href={`/console/onboard/run?${planParam(plan())}`}
								>
									<span class="icon-text">
										<span>Next Step</span>
										<span class="icon">
											<i class="fas fa-chevron-right"></i>
										</span>
									</span>
								</a>
							</div>
						</div>
					</div>
				</div>
			</section>
		</>
	);
};

export default OnboardProject;
