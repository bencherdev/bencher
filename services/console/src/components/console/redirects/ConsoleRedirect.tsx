import bencher_valid_init, { type InitOutput } from "bencher_valid";

import { authUser } from "../../../util/auth";
import {
	NOTIFY_KIND_PARAM,
	NOTIFY_TEXT_PARAM,
	forwardParams,
} from "../../../util/notify";
import { useNavigate, useSearchParams } from "../../../util/url";
import { PLAN_PARAM } from "../../auth/auth";
import { createEffect, createMemo, createResource } from "solid-js";
import { validJwt } from "../../../util/valid";
import { httpGet } from "../../../util/http";
import type { JsonOrganization } from "../../../types/bencher";

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

	const fetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: user.token,
		};
	});
	const getOrganizations = async (fetcher: {
		bencher_valid: InitOutput;
		token: string;
	}) => {
		if (!bencher_valid()) {
			return null;
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
		fetcher,
		getOrganizations,
	);

	createEffect(() => {
		const orgs = organizations();
		if (orgs === undefined || orgs === null) {
			return;
		}
		if (orgs.length !== 1) {
			navigate("/console/organizations");
			return;
		}
		const org = orgs[0] as JsonOrganization;

		const plan = searchParams[PLAN_PARAM];
		console.log(plan);
		if (plan) {
			navigate(
				forwardParams(
					`/console/organizations/${org.slug}/billing`,
					[NOTIFY_KIND_PARAM, NOTIFY_TEXT_PARAM, PLAN_PARAM],
					null,
				),
			);
			return;
		}

		navigate(
			forwardParams(
				`/console/organizations/${org.slug}/projects`,
				[NOTIFY_KIND_PARAM, NOTIFY_TEXT_PARAM],
				null,
			),
		);
	});

	return <></>;
};

export default ConsoleRedirect;
