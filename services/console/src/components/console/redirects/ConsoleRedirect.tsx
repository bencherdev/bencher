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
import type { JsonOrganization } from "../../../types/bencher";
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
		fetcher,
		getOrganizations,
	);

	createEffect(() => {
		const orgs = organizations();
		if (orgs === undefined) {
			return;
		}
		if (orgs === null) {
			removeUser();
			navigate("/auth/login", { replace: true });
			return;
		}
		if (orgs.length !== 1) {
			navigate("/console/organizations", { replace: true });
			return;
		}
		const org = orgs[0] as JsonOrganization;

		const plan = searchParams[PLAN_PARAM];
		if (plan) {
			navigate(
				forwardParams(
					`/console/organizations/${org.slug}/billing`,
					[NOTIFY_KIND_PARAM, NOTIFY_TEXT_PARAM, PLAN_PARAM],
					[
						[
							BACK_PARAM,
							encodeBase64(`/console/organizations/${org.slug}/projects/add`),
						],
					],
				),
				{ replace: true },
			);
			return;
		}

		navigate(
			forwardParams(
				`/console/organizations/${org.slug}/projects`,
				[NOTIFY_KIND_PARAM, NOTIFY_TEXT_PARAM],
				null,
			),
			{ replace: true },
		);
	});

	return <></>;
};

export default ConsoleRedirect;
