import type { Params } from "astro";
import type { InitOutput } from "bencher_valid";
import {
	type Accessor,
	For,
	type Resource,
	type Setter,
	Show,
	createEffect,
	createMemo,
	createSignal,
} from "solid-js";
import {
	type JsonAuthUser,
	type JsonUsage,
	PlanLevel,
} from "../../../../types/bencher";
import { fmtDate } from "../../../../util/convert";
import { useSearchParams } from "../../../../util/url";
import { validPlanLevel, validUuid } from "../../../../util/valid";
import { PLAN_PARAM } from "../../../auth/auth";
import Field from "../../../field/Field";
import FieldKind from "../../../field/kind";
import Pricing from "./Pricing";

// Toggle checkout flow
// import PaymentCard from "./PaymentCard";
import Checkout from "./Checkout";

interface Props {
	apiUrl: string;
	params: Params;
	bencher_valid: Resource<InitOutput>;
	user: JsonAuthUser;
	usage: Resource<null | JsonUsage>;
	handleRefresh: () => void;
}

enum PlanKind {
	Metered,
	Licensed,
	SelfHosted,
}

const BillingForm = (props: Props) => {
	const [searchParams, setSearchParams] = useSearchParams();

	const setPlanLevel = (planLevel: PlanLevel) => {
		setSearchParams({ [PLAN_PARAM]: planLevel });
	};
	const plan = createMemo(() => searchParams[PLAN_PARAM] as PlanLevel);

	const [planKind, setPlanKind] = createSignal(PlanKind.Metered);
	const [entitlements, setEntitlements] = createSignal<number>(6);
	const entitlementsAnnual = createMemo(() => {
		switch (plan()) {
			case PlanLevel.Free:
				return null;
			case PlanLevel.Team:
			case PlanLevel.Enterprise:
				return entitlements() * 10_000;
		}
	});
	const entitlementsAnnualCost = createMemo(() => {
		switch (plan()) {
			case PlanLevel.Free:
				return 0.0;
			case PlanLevel.Team:
				return (entitlementsAnnual() ?? 0.0) * 0.01;
			case PlanLevel.Enterprise:
				return (entitlementsAnnual() ?? 0.0) * 0.05;
		}
	});
	const entitlementsAnnualJson = createMemo(() => {
		switch (planKind()) {
			case PlanKind.Metered:
				return null;
			case PlanKind.Licensed:
			case PlanKind.SelfHosted:
				return entitlementsAnnual();
		}
	});
	const [organizationUuid, setOrganizationUuid] = createSignal("");
	const organizationUuidJson = createMemo(() => {
		switch (planKind()) {
			case PlanKind.Metered:
			case PlanKind.Licensed:
				return null;
			case PlanKind.SelfHosted:
				return organizationUuid();
		}
	});
	const organizationUuidValid = createMemo(() => {
		switch (planKind()) {
			case PlanKind.Metered:
			case PlanKind.Licensed:
				return true;
			case PlanKind.SelfHosted:
				const uuid = organizationUuid();
				if (uuid) {
					return validUuid(uuid) && uuid !== props.usage()?.organization;
				} else {
					return null;
				}
		}
	});
	const organizationUuidValidJson = createMemo(
		() => organizationUuidValid() === true,
	);

	createEffect(() => {
		if (!props.bencher_valid()) {
			return;
		}
		if (!validPlanLevel(searchParams[PLAN_PARAM])) {
			setPlanLevel(PlanLevel.Free);
		}
	});

	return (
		<>
			<Pricing
				plan={plan()}
				freeText="Stick with Free"
				handleFree={() => setPlanLevel(PlanLevel.Free)}
				teamText="Go with Team"
				handleTeam={() => setPlanLevel(PlanLevel.Team)}
				enterpriseText="Go with Enterprise"
				handleEnterprise={() => setPlanLevel(PlanLevel.Enterprise)}
			/>
			<Show
				when={plan() !== PlanLevel.Free}
				fallback={<FreeUsage usage={props.usage} />}
			>
				<PlanLocality
					plan={plan}
					planKind={planKind}
					handlePlanKind={setPlanKind}
					entitlements={entitlements}
					handleEntitlements={setEntitlements}
					entitlementsAnnual={entitlementsAnnual}
					entitlementsAnnualCost={entitlementsAnnualCost}
					organizationUuid={organizationUuid}
					handleOrganizationUuid={setOrganizationUuid}
					organizationUuidValid={organizationUuidValid}
				/>
				{/* <PaymentCard
					apiUrl={props.apiUrl}
					params={props.params}
					bencher_valid={props.bencher_valid}
					user={props.user}
					path={`/v0/organizations/${props.params.organization}/plan`}
					plan={plan}
					entitlements={entitlementsAnnualJson}
					organizationUuid={organizationUuidJson}
					organizationUuidValid={organizationUuidValidJson}
					handleRefresh={props.handleRefresh}
				/> */}
				<Checkout
					apiUrl={props.apiUrl}
					params={props.params}
					bencher_valid={props.bencher_valid}
					user={props.user}
					organization={props.usage()?.organization}
					plan={plan}
					entitlements={entitlementsAnnualJson}
					organizationUuid={organizationUuidJson}
					organizationUuidValid={organizationUuidValidJson}
					handleRefresh={props.handleRefresh}
				/>
			</Show>
		</>
	);
};

const FreeUsage = (props: { usage: Resource<null | JsonUsage> }) => {
	return (
		<div class="columns">
			<div class="column">
				<div class="content" style="margin-top: 4rem">
					<h2 class="title is-2">Free Tier Usage</h2>
					<h3 class="subtitle is-3">
						{fmtDate(props.usage()?.start_time)} -{" "}
						{fmtDate(props.usage()?.end_time)}
					</h3>
					<h4>Metrics Used: {props.usage()?.usage?.toLocaleString() ?? 0}</h4>
				</div>
			</div>
		</div>
	);
};

const PlanLocality = (props: {
	plan: Accessor<PlanLevel>;
	planKind: Accessor<PlanKind>;
	handlePlanKind: Setter<PlanKind>;
	entitlements: Accessor<number>;
	handleEntitlements: Setter<number>;
	entitlementsAnnual: Accessor<null | number>;
	entitlementsAnnualCost: Accessor<number>;
	organizationUuid: Accessor<string>;
	handleOrganizationUuid: Setter<string>;
	organizationUuidValid: Accessor<null | boolean>;
}) => {
	return (
		<div class="columns is-centered">
			<div class="column">
				<div class="buttons has-addons is-centered" style="margin-top: 4rem">
					<For
						each={[
							["Monthly Metered", PlanKind.Metered],
							["Annual License", PlanKind.Licensed],
							["Self-Hosted License", PlanKind.SelfHosted],
						]}
					>
						{([name, kind]) => (
							<button
								class={`button ${
									props.planKind() === kind && "is-primary is-selected"
								}`}
								onClick={(_e) => props.handlePlanKind(kind as PlanKind)}
							>
								{name}
							</button>
						)}
					</For>
				</div>
				<Show when={props.planKind() !== PlanKind.Metered}>
					<div class="content has-text-centered">
						<p>
							Annual Metrics:{" "}
							{props.entitlementsAnnual()?.toLocaleString() ?? 0}
						</p>
						<p>
							Annual Cost: ${props.entitlementsAnnualCost().toLocaleString()}
						</p>
						<input
							class="slider"
							type="range"
							min="1"
							max="100"
							value={props.entitlements()}
							style="width: 50%"
							onChange={(_e) => {
								props.handleEntitlements(Number(_e.currentTarget.value));
							}}
						></input>
					</div>
				</Show>
				<Show when={props.planKind() === PlanKind.SelfHosted}>
					<div class="columns is-centered">
						<div class="column is-half">
							<Field
								kind={FieldKind.INPUT}
								fieldKey="organization-uuid"
								label="Self-Hosted Organization UUID"
								value={props.organizationUuid()}
								valid={props.organizationUuidValid()}
								config={{
									label: "UUID",
									type: "text",
									placeholder: "00000000-0000-0000-0000-000000000000",
									icon: "fas fa-laptop-house",
									help: "Must be a valid UUID for a Self-Hosted Organization",
									validate: validUuid,
								}}
								handleField={(_key, value, _valid) => {
									props.handleOrganizationUuid(value as string);
								}}
							/>
						</div>
					</div>
				</Show>
			</div>
		</div>
	);
};

export default BillingForm;
