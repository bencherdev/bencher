import * as Sentry from "@sentry/astro";
import { createEffect, createMemo, createResource } from "solid-js";
import type { JsonOrganization, PlanLevel } from "../../../types/bencher";
import { authUser } from "../../../util/auth";
import { httpGet } from "../../../util/http";
import { getOrganization, setOrganization } from "../../../util/organization";
import { useSearchParams } from "../../../util/url";
import {
	type InitValid,
	init_valid,
	validJwt,
	validPlanLevel,
} from "../../../util/valid";
import { PLAN_PARAM } from "../../auth/auth";
import BillingPanel from "../billing/BillingPanel";
import OnboardSteps from "./OnboardSteps";
import { OnboardStep } from "./OnboardStepsInner";

export interface Props {
	apiUrl: string;
	isBencherCloud: boolean;
}

const OnboardPlan = (props: Props) => {
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
								isBencherCloud={props.isBencherCloud}
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
