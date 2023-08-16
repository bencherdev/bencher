import bencher_valid_init from "bencher_valid";
import {
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";

import type { FieldHandler } from "../field/Field";
import Field from "../field/Field";
import FieldKind from "../field/kind";
import { AUTH_FIELDS, EMAIL_PARAM, PLAN_PARAM, TOKEN_PARAM } from "./auth";
import { useSearchParams } from "../../util/url";
import { validEmail, validJwt, validPlanLevel } from "../../util/valid";
import { createStore } from "solid-js/store";
import { BENCHER_API_URL } from "../../util/ext";
import { httpPost } from "../../util/http";
import { setUser } from "../../util/auth";
import type { Email, Jwt, PlanLevel } from "../../types/bencher";
import { NotifyKind, navigateNotify } from "../../util/notify";

export interface Props {}

const ConfirmForm = (_props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);

	const [searchParams, setSearchParams] = useSearchParams();
	const [submitting, setSubmitting] = createSignal(false);
	const [valid, setValid] = createSignal(false);

	const isSendable = (): boolean => {
		return !submitting() && valid();
	};

	const token = createMemo(() => searchParams[TOKEN_PARAM]?.trim() as Jwt);
	const plan = createMemo(() => searchParams[PLAN_PARAM]?.trim() as PlanLevel);
	const email = createMemo(() => searchParams[EMAIL_PARAM]?.trim() as Email);

	const [submitted, setSubmitted] = createSignal();
	const [form, setForm] = createStore<{
		token: {
			value: string;
			valid: null | boolean;
		};
	}>(initForm());

	const [coolDown, setCoolDown] = createSignal(true);
	setTimeout(() => setCoolDown(false), 10000);

	const handleField: FieldHandler = (key, value, valid) => {
		setForm({
			...form,
			[key]: {
				value: value,
				valid: valid,
			},
		});
	};

	const post = async () => {
		const url = `${BENCHER_API_URL()}/v0/auth/confirm`;
		const no_token = null;
		const data = {
			token: token(),
		};
		return await httpPost(url, no_token, data);
	};

	const handleSubmit = () => {
		setSubmitting(true);

		post()
			.then((resp) => {
				setSubmitting(false);
				const user = resp.data;
				if (setUser(user)) {
					navigateNotify(
						NotifyKind.OK,
						`Ahoy, ${user.user.name}!`,
						"/console",
						[PLAN_PARAM],
						null,
					);
				} else {
					navigateNotify(
						NotifyKind.ERROR,
						"Invalid user. Please, try again.",
						null,
						[PLAN_PARAM, EMAIL_PARAM],
						null,
					);
				}
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				navigateNotify(
					NotifyKind.ERROR,
					"Failed to confirm token. Please, try again.",
					null,
					[PLAN_PARAM, EMAIL_PARAM],
					null,
				);
			});
	};

	const handleResendEmail = () => {
		setSubmitting(true);

		const data = {
			email: email().trim(),
			plan: plan()?.trim(),
		};

		const url = `${BENCHER_API_URL()}/v0/auth/login`;
		httpPost(url, null, data)
			.then((_resp) => {
				setSubmitting(false);
				navigateNotify(
					NotifyKind.OK,
					`Successful resent email to ${email()}. Please confirm token.`,
					null,
					[PLAN_PARAM, EMAIL_PARAM],
					null,
				);
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				navigateNotify(
					NotifyKind.ERROR,
					`Failed to resend email to ${email()}. Please, try again.`,
					null,
					[PLAN_PARAM, EMAIL_PARAM],
					null,
				);
			});

		setCoolDown(true);
		setTimeout(() => setCoolDown(false), 30000);
	};

	createEffect(() => {
		if (!bencher_valid()) {
			return;
		}

		const newParams: Record<string, null | string> = {};
		if (!validJwt(searchParams[TOKEN_PARAM])) {
			newParams[TOKEN_PARAM] = null;
		}
		if (!validPlanLevel(searchParams[PLAN_PARAM])) {
			newParams[PLAN_PARAM] = null;
		}
		if (!validEmail(searchParams[EMAIL_PARAM])) {
			newParams[EMAIL_PARAM] = null;
		}
		const token_value = form.token?.value;
		if (validJwt(token_value)) {
			newParams[TOKEN_PARAM] = token_value;
		}
		if (Object.keys(newParams).length !== 0) {
			setSearchParams(newParams);
		}

		const token_valid = form.token?.valid;
		if (typeof token_valid === "boolean" && token_valid !== valid()) {
			setValid(token_valid);
		}

		const jwt = token();
		if (validJwt(jwt) && jwt !== submitted()) {
			setSubmitted(jwt);
			handleSubmit();
		}
	});

	return (
		<>
			<form class="box">
				<Field
					kind={FieldKind.INPUT}
					fieldKey="token"
					value={form?.token?.value}
					valid={form?.token?.valid}
					config={AUTH_FIELDS.token}
					handleField={handleField}
				/>

				<div class="field">
					<p class="control">
						<button
							class="button is-primary is-fullwidth"
							disabled={!isSendable()}
							onClick={(e) => {
								e.preventDefault();
								handleSubmit();
							}}
						>
							Submit
						</button>
					</p>
				</div>
			</form>

			{email() && (
				<>
					<hr />

					<div class="content has-text-centered">
						<button
							class="button is-small is-black is-inverted"
							disabled={submitting() || coolDown()}
							onClick={(e) => {
								e.preventDefault();
								handleResendEmail();
							}}
						>
							<div>Click to resend email to: {email()}</div>
						</button>
					</div>
				</>
			)}
		</>
	);
};

const initForm = () => {
	return {
		token: {
			value: "",
			valid: null,
		},
	};
};

export default ConfirmForm;
