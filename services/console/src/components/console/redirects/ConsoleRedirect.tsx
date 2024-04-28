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
import type {
	JsonOrganization,
	JsonProject,
	JsonToken,
} from "../../../types/bencher";
import { encodeBase64 } from "../../../util/convert";

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

	const tokensFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: user.token,
		};
	});
	const getTokens = async (fetcher: {
		bencher_valid: InitOutput;
		token: string;
	}) => {
		if (!fetcher.bencher_valid) {
			return undefined;
		}
		if (!validJwt(fetcher.token)) {
			return null;
		}
		const path = `/v0/users/${user?.user?.uuid}/tokens`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				return null;
			});
	};
	const [apiTokens] = createResource<null | JsonToken[]>(
		tokensFetcher,
		getTokens,
	);

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

	const organization = createMemo(
		() => organizations()?.[0] as JsonOrganization,
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

	createEffect(() => {
		const tokens = apiTokens();
		if (tokens === undefined) {
			return;
		}
		if (tokens === null || tokens.length === 0) {
			navigate("/console/onboard/tokens", { replace: true });
			return;
		}

		// const orgs = organizations();
		// // Wait for wasm to load
		// if (orgs === undefined) {
		// 	return;
		// }
		// // If there is no token, redirect to login
		// if (orgs === null) {
		// 	removeUser();
		// 	navigate("/auth/login", { replace: true });
		// 	return;
		// }
		// // If there is more than one organization, then redirect to the organizations table
		// if (orgs.length !== 1) {
		// 	navigate("/console/organizations", { replace: true });
		// 	return;
		// }

		// const org = organization();
		// if (!org) {
		// 	return;
		// }
		// console.log("org", org);
		// console.log("projects", projects());
		// const ps = projects();
		// // Wait for wasm to load
		// if (ps === undefined) {
		// 	return;
		// }
		// // If there is no token, redirect to login
		// if (ps === null || ps.length === 0) {
		// 	navigate("/console/onboard", { replace: true });
		// 	return;
		// }

		// const plan = searchParams[PLAN_PARAM];
		// if (plan) {
		// 	navigate(
		// 		forwardParams(
		// 			`/console/organizations/${org.slug}/billing`,
		// 			[NOTIFY_KIND_PARAM, NOTIFY_TEXT_PARAM, PLAN_PARAM],
		// 			[
		// 				[
		// 					BACK_PARAM,
		// 					encodeBase64(`/console/organizations/${org.slug}/projects/add`),
		// 				],
		// 			],
		// 		),
		// 		{ replace: true },
		// 	);
		// 	return;
		// }

		// navigate(
		// 	forwardParams(
		// 		`/console/organizations/${org.slug}/projects`,
		// 		[NOTIFY_KIND_PARAM, NOTIFY_TEXT_PARAM],
		// 		null,
		// 	),
		// 	{ replace: true },
		// );
	});

	return <></>;
};

export default ConsoleRedirect;
