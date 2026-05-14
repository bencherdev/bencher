import * as Sentry from "@sentry/astro";
import { createEffect, createMemo, createResource } from "solid-js";
import type {
	JsonNewProjectKey,
	JsonOrganization,
	JsonProject,
	JsonProjectKey,
	JsonProjectKeyCreated,
	PlanLevel,
} from "../../../types/bencher";
import { authUser } from "../../../util/auth";
import { httpGet, httpPost } from "../../../util/http";
import {
	getOnboardProjectKey,
	setOnboardProjectKey,
} from "../../../util/onboard";
import { getOrganization, setOrganization } from "../../../util/organization";
import { useSearchParams } from "../../../util/url";
import {
	type InitValid,
	init_valid,
	validJwt,
	validPlanLevel,
} from "../../../util/valid";
import { PLAN_PARAM, planParam } from "../../auth/auth";
import CopyButton from "./CopyButton";
import OnboardSteps from "./OnboardSteps";
import { OnboardStep } from "./OnboardStepsInner";

export interface Props {
	apiUrl: string;
}

const OnboardKey = (props: Props) => {
	const [bencher_valid] = createResource(init_valid);
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
		bencher_valid: InitValid;
		token: string;
	}) => {
		const cachedOrganization = getOrganization();
		if (cachedOrganization) {
			return [cachedOrganization];
		}
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
				Sentry.captureException(error);
				return;
			});
	};
	const [organizations] = createResource<undefined | JsonOrganization[]>(
		orgsFetcher,
		getOrganizations,
	);

	const organization = createMemo(() => {
		const orgs = organizations();
		if (Array.isArray(orgs) && (orgs?.length ?? 0) > 0) {
			const org = orgs?.[0] as JsonOrganization;
			if (orgs.length === 1) {
				setOrganization(org);
			}
			return org;
		}
		return undefined;
	});

	const projectsFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: user.token,
			organization: organization(),
		};
	});
	const getProjects = async (fetcher: {
		bencher_valid: InitValid;
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
				Sentry.captureException(error);
				return;
			});
	};
	const [projects] = createResource<undefined | JsonProject[]>(
		projectsFetcher,
		getProjects,
	);

	const project = createMemo(() => {
		const orgProjects = projects();
		return Array.isArray(orgProjects) && (orgProjects?.length ?? 0) > 0
			? (orgProjects?.[0] as JsonProject)
			: undefined;
	});

	const keyName = createMemo(() => `${user?.user?.name}'s key`);

	const keysFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: user.token,
			project: project(),
		};
	});
	const getKeys = async (fetcher: {
		bencher_valid: InitValid;
		token: string;
		project: undefined | JsonProject;
	}) => {
		if (!fetcher.bencher_valid) {
			return;
		}
		if (!validJwt(fetcher.token) || fetcher.project === undefined) {
			return;
		}
		const path = `/v0/projects/${
			fetcher.project?.slug
		}/keys?name=${encodeURIComponent(keyName())}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return;
			});
	};
	const [apiKeys] = createResource<undefined | JsonProjectKey[]>(
		keysFetcher,
		getKeys,
	);

	const keyFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: user.token,
			project: project(),
			keys: apiKeys(),
		};
	});
	const getKey = async (fetcher: {
		bencher_valid: InitValid;
		token: string;
		project: undefined | JsonProject;
		keys: undefined | JsonProjectKey[];
	}) => {
		if (!fetcher.bencher_valid) {
			return;
		}
		if (
			!validJwt(fetcher.token) ||
			fetcher.project === undefined ||
			fetcher.keys === undefined
		) {
			return;
		}
		if (fetcher.keys.length > 0) {
			return fetcher.keys[0];
		}
		const path = `/v0/projects/${fetcher.project?.slug}/keys`;
		const data: JsonNewProjectKey = {
			name: keyName(),
		};
		return await httpPost(props.apiUrl, path, fetcher.token, data)
			.then((resp) => {
				const created = resp?.data as JsonProjectKeyCreated;
				if (created?.key) {
					setOnboardProjectKey(created.key);
				}
				return created;
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return;
			});
	};
	const [apiKey] = createResource<
		undefined | JsonProjectKey | JsonProjectKeyCreated
	>(keyFetcher, getKey);

	const keyValue = createMemo(() => {
		const key = apiKey();
		if (key && "key" in key) {
			return (key as JsonProjectKeyCreated).key;
		}
		return getOnboardProjectKey() ?? "";
	});

	return (
		<>
			<OnboardSteps step={OnboardStep.KEY} plan={plan} />

			<section class="section">
				<div class="container">
					<div class="columns is-centered">
						<div class="column is-half">
							<div class="content has-text-centered">
								<h1 class="title is-1">Use this project API key</h1>
								<h2 class="subtitle is-4">
									Authenticate with Bencher using this project API key.
								</h2>
								<article class="message is-warning">
									<div class="message-body">
										Save this key! It will only be shown during onboarding.
									</div>
								</article>
								<figure class="frame">
									<pre data-language="plaintext">
										<code>
											<div class="code">{keyValue()}</div>
										</code>
									</pre>
									<CopyButton text={keyValue()} />
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

export default OnboardKey;
