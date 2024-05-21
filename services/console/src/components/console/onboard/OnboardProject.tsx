import bencher_valid_init, { type InitOutput, new_slug } from "bencher_valid";

import {
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import { authUser } from "../../../util/auth";
import { useSearchParams } from "../../../util/url";
import {
	validJwt,
	validPlanLevel,
	validResourceName,
} from "../../../util/valid";
import { httpGet, httpPatch, httpPost } from "../../../util/http";
import type {
	JsonNewProject,
	JsonOrganization,
	JsonProject,
	PlanLevel,
} from "../../../types/bencher";
import Field, { type FieldHandler } from "../../field/Field";
import FieldKind from "../../field/kind";
import { PLAN_PARAM, planParam } from "../../auth/auth";
import OnboardSteps from "./OnboardSteps";
import CopyButton from "./CopyButton";
import { OnboardStep } from "./OnboardStepsInner";

export interface Props {
	apiUrl: string;
}

const OnboardProject = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const user = authUser();
	const [searchParams, setSearchParams] = useSearchParams();

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
			return;
		}
		if (!validJwt(fetcher.token)) {
			return;
		}
		const path = "/v0/organizations";
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				return;
			});
	};
	const [organizations] = createResource<undefined | JsonOrganization[]>(
		orgsFetcher,
		getOrganizations,
	);

	const organization = createMemo(() => {
		const orgs = organizations();
		return Array.isArray(orgs) && (orgs?.length ?? 0) > 0
			? (orgs?.[0] as JsonOrganization)
			: undefined;
	});

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
			return;
		}
		if (!validJwt(fetcher.token) || fetcher.organization === undefined) {
			return;
		}
		const path = `/v0/organizations/${fetcher.organization?.slug}/projects`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				return;
			});
	};
	const [projects, { refetch: refetchProjects }] = createResource<
		undefined | JsonProject[]
	>(projectsFetcher, getProjects);

	const organizationProject = createMemo(() => {
		const orgProjects = projects();
		return Array.isArray(orgProjects) && (orgProjects?.length ?? 0) > 0
			? (orgProjects?.[0] as JsonProject)
			: undefined;
	});

	const projectFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: user.token,
			organization: organization(),
			projects: projects(),
			project: organizationProject(),
		};
	});
	const getProject = async (fetcher: {
		bencher_valid: InitOutput;
		token: string;
		organization: undefined | JsonOrganization;
		projects: undefined | JsonProject[];
		project: undefined | JsonProject;
	}) => {
		if (!fetcher.bencher_valid) {
			return;
		}
		if (
			!validJwt(fetcher.token) ||
			fetcher.organization === undefined ||
			fetcher.projects === undefined
		) {
			return;
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
				return;
			});
	};
	const [project] = createResource<undefined | JsonProject>(
		projectFetcher,
		getProject,
	);

	const [renameProject, setRenameProject] = createSignal<null | string>(null);
	const [renameValid, setRenameValid] = createSignal<null | boolean>(null);
	const [submitting, setSubmitting] = createSignal(false);

	const isSendable = (): boolean =>
		!submitting() &&
		renameProject() !== project()?.name &&
		(renameValid() ?? false);

	const handleField: FieldHandler = (_key, value, valid) => {
		setRenameProject(value as string);
		setRenameValid(valid as boolean);
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
		if (!fetcher.submitting || !fetcher.bencher_valid) {
			setSubmitting(false);
			return undefined;
		}
		if (
			!validJwt(fetcher.token) ||
			fetcher.project === undefined ||
			fetcher.renameProject === null ||
			fetcher.renameValid === null ||
			fetcher.renameValid === false
		) {
			setSubmitting(false);
			return null;
		}
		const path = `/v0/projects/${fetcher.project?.slug}`;
		const data = {
			name: fetcher.renameProject,
			slug: new_slug(fetcher.renameProject),
		};
		return await httpPatch(props.apiUrl, path, fetcher.token, data)
			.then((resp) => {
				setSubmitting(false);
				refetchProjects();
				return resp?.data;
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				return;
			});
	};
	const [_updatedProject] = createResource<undefined | JsonProject>(
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
										class="button is-primary is-fullwidth"
										title="Save project name"
										disabled={!isSendable()}
										onClick={(e) => {
											e.preventDefault();
											setSubmitting(true);
										}}
									>
										Save Project Name
									</button>
								</form>
								<br />
								<figure class="frame">
									<h3 class="title is-5">This is your project slug</h3>
									<pre data-language="plaintext">
										<code>
											<div class="code">{project()?.slug}</div>
										</code>
									</pre>
									<CopyButton text={project()?.slug ?? ""} />
								</figure>
								<br />
								<br />

								<a
									class="button is-primary is-fullwidth"
									href={`/console/onboard/run${planParam(plan())}`}
								>
									<span class="icon-text">
										<span>Next Step</span>
										<span class="icon">
											<i class="fas fa-chevron-right" />
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
