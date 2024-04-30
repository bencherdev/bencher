import bencher_valid_init, { type InitOutput } from "bencher_valid";

import { authUser, removeUser } from "../../../util/auth";
import {
	NOTIFY_KIND_PARAM,
	NOTIFY_TEXT_PARAM,
	forwardParams,
} from "../../../util/notify";
import { BACK_PARAM, useNavigate, useSearchParams } from "../../../util/url";
import { PLAN_PARAM } from "../../auth/auth";
import { createEffect, createMemo, createResource } from "solid-js";
import { validJwt } from "../../../util/valid";
import { httpGet } from "../../../util/http";
import type { JsonOrganization, JsonProject } from "../../../types/bencher";
import { encodeBase64 } from "../../../util/convert";
import { log } from "mermaid/dist/logger.js";

export interface Props {
	apiUrl: string;
}

const ConsoleRedirect = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const user = authUser();
	const [searchParams, _setSearchParams] = useSearchParams();
	const navigate = useNavigate();

	const logout = () => {
		removeUser();
		navigate("/auth/login", { replace: true });
	}

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
		const path = "/v0/organizations?per_page=2";
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
		if (!validJwt(fetcher.token)) {
			logout();
			return;
		}
		if (organizations.loading || (organizations()?.length ?? 0) > 1 ||fetcher.organization === undefined) {
			return;
		}
		const path = `/v0/organizations/${fetcher.organization?.slug}/projects?per_page=1`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				logout();
				return;
			});
	};
	const [projects] = createResource<
		undefined | JsonProject[]
	>(projectsFetcher, getProjects);

	createEffect(() => {
		if (organizations.loading) {
			return;
		}
		const orgs = organizations();
		if (orgs === undefined) {
			return;
		}
		if ((orgs?.length ?? 0) > 1) {
			navigate("/console/organizations", { replace: true });
			return;
		}
		if (projects.loading) {
			return;
		}
		const projs = projects();
		if (projs === undefined) {
			return;
		}
		if ((projects()?.length ?? 0) === 0 || searchParams[PLAN_PARAM]) {
			navigate(
				forwardParams(
					`/console/onboard/token`,
					[PLAN_PARAM],
					[],
				),
				{ replace: true },
			);
			return;
		}

		const org = organization();
		if (org === undefined) {
			logout();
			return;
		}
		navigate(
			forwardParams(
				`/console/organizations/${org?.slug}/projects`,
				[NOTIFY_KIND_PARAM, NOTIFY_TEXT_PARAM],
				null,
			),
			{ replace: true },
		);
	});

	return <></>;
};

export default ConsoleRedirect;
