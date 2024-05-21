import bencher_valid_init, { type InitOutput } from "bencher_valid";

import {
	For,
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import { authUser } from "../../../util/auth";
import { useSearchParams } from "../../../util/url";
import { validJwt, validPlanLevel } from "../../../util/valid";
import { httpGet, httpPost } from "../../../util/http";
import {
	OrganizationRole,
	type JsonAuthAck,
	type JsonNewMember,
	type JsonOrganization,
	type PlanLevel,
} from "../../../types/bencher";
import Field, { type FieldHandler } from "../../field/Field";
import FieldKind from "../../field/kind";
import { PLAN_PARAM, planParam } from "../../auth/auth";
import OnboardSteps from "./OnboardSteps";
import { createStore } from "solid-js/store";
import { MEMBER_FIELDS } from "../../../config/organization/members";
import { OnboardStep } from "./OnboardStepsInner";

export interface Props {
	apiUrl: string;
}

const OnboardInvite = (props: Props) => {
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

	const [invited, setInvited] = createStore<JsonNewMember[]>([]);
	const [form, setForm] = createStore(initForm());
	const [submitting, setSubmitting] = createSignal(false);
	const [valid, setValid] = createSignal(false);

	const isSendable = (): boolean => {
		return !submitting() && valid();
	};

	const handleField: FieldHandler = (key, value, valid) => {
		setForm({
			...form,
			[key]: {
				value,
				valid,
			},
		});

		setValid(validateForm());
	};

	const validateForm = () => (form?.name?.valid && form?.email?.valid) ?? false;

	const inviteFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: user.token,
			organization: organization(),
			form: form,
			submitting: submitting(),
			valid: valid(),
		};
	});
	const postInvite = async (fetcher: {
		bencher_valid: InitOutput;
		token: string;
		organization: undefined | JsonOrganization;
		form: object;
		submitting: boolean;
		valid: boolean;
	}) => {
		if (!fetcher.bencher_valid) {
			setSubmitting(false);
			return;
		}
		if (
			!validJwt(fetcher.token) ||
			!fetcher.organization ||
			!fetcher.submitting ||
			!fetcher.valid
		) {
			setSubmitting(false);
			return;
		}
		const path = `/v0/organizations/${fetcher.organization?.slug}/members`;
		const data: JsonNewMember = {
			name: fetcher.form?.name?.value,
			email: fetcher.form?.email?.value,
			role: OrganizationRole.Leader,
		};
		return await httpPost(props.apiUrl, path, fetcher.token, data)
			.then((resp) => {
				setInvited([...invited, data]);
				setForm(initForm());
				setSubmitting(false);
				return resp?.data;
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				return null;
			});
	};
	const [_invite] = createResource<undefined | JsonAuthAck>(
		inviteFetcher,
		postInvite,
	);

	return (
		<>
			<OnboardSteps step={OnboardStep.INVITE} plan={plan} />

			<section class="section">
				<div class="container">
					<div class="columns is-centered">
						<div class="column is-half">
							<div class="content has-text-centered">
								<h1 class="title is-1">Invite your collaborators</h1>
								<h2 class="subtitle is-4">
									Add collaborators as members of your organization.
								</h2>
								<For each={invited}>
									{(invite) => (
										<div class="box">
											<p style="word-break: break-all;">
												<b>Invited:</b> {invite.name} &lt;{invite.email}&gt;
											</p>
										</div>
									)}
								</For>
								<form
									onSubmit={(e) => {
										e.preventDefault();
									}}
								>
									<Field
										kind={FieldKind.INPUT}
										fieldKey="name"
										value={form?.name?.value}
										valid={form?.name?.valid}
										config={MEMBER_FIELDS.name}
										handleField={handleField}
									/>
									<Field
										kind={FieldKind.INPUT}
										fieldKey="email"
										value={form?.email?.value}
										valid={form?.email?.valid}
										config={MEMBER_FIELDS.email}
										handleField={handleField}
									/>
									<button
										class="button is-primary is-fullwidth"
										title="Send invite to collaborator"
										disabled={!isSendable()}
										onClick={(e) => {
											e.preventDefault();
											setSubmitting(true);
										}}
									>
										Send Invite
									</button>
								</form>
								<br />
								<br />
								<a
									class="button is-primary is-fullwidth"
									href={`/console/onboard/plan${planParam(plan())}`}
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

const initForm = () => {
	return {
		name: {
			value: "",
			valid: null,
		},
		email: {
			value: "",
			valid: null,
		},
	};
};

export default OnboardInvite;
