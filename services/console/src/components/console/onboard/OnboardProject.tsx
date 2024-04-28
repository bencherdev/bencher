import bencher_valid_init, { type InitOutput } from "bencher_valid";

import {
	Match,
	Switch,
	createEffect,
	createMemo,
	createResource,
} from "solid-js";
import { authUser } from "../../../util/auth";
import { useNavigate, useSearchParams } from "../../../util/url";
import { validJwt } from "../../../util/valid";
import { httpGet, httpPost } from "../../../util/http";
import type {
	JsonNewProject,
	JsonNewToken,
	JsonOrganization,
	JsonProject,
	JsonToken,
} from "../../../types/bencher";

export interface Props {
	apiUrl: string;
}

const OnboardProject = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const user = authUser();
	const [searchParams, _setSearchParams] = useSearchParams();
	const navigate = useNavigate();

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
		Array.isArray(organizations()) && (organizations()?.length ?? 0) >= 1
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
	const [projects] = createResource<null | JsonProject[]>(
		projectsFetcher,
		getProjects,
	);

	const organizationProject = createMemo(() =>
		Array.isArray(projects()) && (projects()?.length ?? 0) >= 1
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
	const [project] = createResource<null | JsonProject[]>(
		projectFetcher,
		getProject,
	);

	return (
		<>
			<figure class="frame">
				<h3 class="title is-5">Use this slug for your project:</h3>
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

			<a class="button is-primary is-fullwidth" href="/console/onboard/project">
				<span class="icon-text">
					<span>Next Step</span>
					<span class="icon">
						<i class="fas fa-chevron-right"></i>
					</span>
				</span>
			</a>
		</>
	);
};

export default OnboardProject;
