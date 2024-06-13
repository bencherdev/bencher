import bencher_valid_init, { type InitOutput } from "bencher_valid";

import { createEffect, createMemo, createResource } from "solid-js";
import { authUser } from "../../../util/auth";
import { useSearchParams } from "../../../util/url";
import { validJwt, validPlanLevel } from "../../../util/valid";
import { httpGet } from "../../../util/http";
import type { JsonOrganization, PlanLevel } from "../../../types/bencher";
import { PLAN_PARAM } from "../../auth/auth";
import OnboardSteps from "./OnboardSteps";
import BillingPanel from "../billing/BillingPanel";
import { OnboardStep } from "./OnboardStepsInner";

export interface Props {
	apiUrl: string;
}

const OnboardPlan = (props: Props) => {
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

	return (
		<>
			<OnboardSteps step={OnboardStep.PLAN} plan={plan} />

			<section class="section">
				<div class="container">
					<div class="columns is-centered">
						<div class="column is-half">
							<div class="content has-text-centered">
								<h1 class="title is-1">Activate your account</h1>
								<h2 class="subtitle is-4">
									All plans come with a 60-day money-back guarantee.
								</h2>
							</div>
							<br />
							<BillingPanel
								apiUrl={props.apiUrl}
								params={{ organization: organization()?.slug ?? "" }}
								onboard={true}
							/>
							<br />
							<div class="content has-text-centered">
								<a class="button" href="/console">
									I want to use the free plan for now
								</a>
							</div>
							<br />
						</div>
					</div>
				</div>
			</section>
		</>
	);
};

export default OnboardPlan;
