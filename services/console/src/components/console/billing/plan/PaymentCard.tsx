import type { Params } from "astro";
import type { InitOutput } from "bencher_valid";
import { type Accessor, type Resource, Show, createSignal } from "solid-js";
import { createStore } from "solid-js/store";
import type {
	JsonAuthUser,
	JsonCustomer,
	JsonNewPayment,
	JsonNewPlan,
	PlanLevel,
} from "../../../../types/bencher";
import { httpPost } from "../../../../util/http";
import { NotifyKind, pageNotify } from "../../../../util/notify";
import {
	cleanCardNumber,
	cleanExpiration,
	validCardCvc,
	validCardNumber,
	validExpiration,
	validJwt,
	validNonEmpty,
} from "../../../../util/valid";
import Field, { type FieldValue } from "../../../field/Field";
import FieldKind from "../../../field/kind";

interface Props {
	apiUrl: string;
	params: Params;
	bencher_valid: Resource<InitOutput>;
	user: JsonAuthUser;
	path: string;
	plan: Accessor<PlanLevel>;
	entitlements: Accessor<null | number>;
	organizationUuid: Accessor<null | string>;
	organizationUuidValid: Accessor<boolean>;
	handleRefresh: () => void;
}

const PaymentCard = (props: Props) => {
	const [form, setForm] = createStore(initCardFrom());
	const [submitting, setSubmitting] = createSignal(false);

	const isSendable = (): boolean => {
		return (
			!submitting() &&
			formValid() &&
			(form?.consent?.valid ?? false) &&
			props.organizationUuidValid()
		);
	};
	const formValid = (): boolean => {
		if (
			form?.name?.valid &&
			form?.number?.valid &&
			form?.expiration?.valid &&
			form?.cvc?.valid
		) {
			return true;
		}
		return false;
	};

	const handleField = (key: string, value: FieldValue, valid: boolean) => {
		setForm({
			...form,
			[key]: {
				value,
				valid,
			},
		});
	};

	const sendForm = () => {
		if (!props.bencher_valid()) {
			return;
		}
		const token = props.user?.token;
		if (!validJwt(token)) {
			return;
		}
		if (!isSendable()) {
			return;
		}

		const customer: JsonCustomer = {
			uuid: props.user?.user?.uuid,
			name: form?.name?.value,
			email: props.user?.user?.email,
		};
		const number = cleanCardNumber(form?.number?.value);
		const exp = cleanExpiration(form?.expiration?.value);
		if (exp === null) {
			return;
		}
		const cvc = form?.cvc?.value?.trim();
		const card = {
			number: number,
			exp_month: exp[0],
			exp_year: exp[1],
			cvc: cvc,
		};
		const newPayment: JsonNewPayment = {
			customer,
			card,
		};

		setSubmitting(true);
		httpPost(props.apiUrl, "/v0/payments", token, newPayment)
			.then((payment) => {
				const newPlan: JsonNewPlan = {
					customer: payment?.data?.customer,
					payment_method: payment?.data?.payment_method,
					level: props.plan(),
					entitlements: props.entitlements(),
					organization: props.organizationUuid(),
					i_agree: form?.consent?.value,
				};
				httpPost(props.apiUrl, props.path, token, newPlan)
					.then((_resp) => {
						setSubmitting(false);
						props.handleRefresh();
						pageNotify(
							NotifyKind.OK,
							"Somebunny loves us! Successful plan enrollment.",
						);
					})
					.catch((error) => {
						setSubmitting(false);
						console.error(error);
						pageNotify(
							NotifyKind.ERROR,
							"Lettuce romaine calm! Failed to enroll. Please, try again.",
						);
					});
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				pageNotify(
					NotifyKind.ERROR,
					"Lettuce romaine calm! Failed to enroll. Please, try again.",
				);
			});
	};

	return (
		<form class="box">
			<Field
				params={props.params}
				kind={FieldKind.INPUT}
				fieldKey="name"
				label={CARD_FIELDS.name?.label}
				value={form?.name?.value}
				valid={form?.name?.valid}
				config={CARD_FIELDS.name}
				handleField={handleField}
			/>
			<Field
				params={props.params}
				kind={FieldKind.INPUT}
				fieldKey="number"
				label={CARD_FIELDS.number?.label}
				value={form?.number?.value}
				valid={form?.number?.valid}
				config={CARD_FIELDS.number}
				handleField={handleField}
			/>
			<Field
				params={props.params}
				kind={FieldKind.INPUT}
				fieldKey="expiration"
				label={CARD_FIELDS.expiration?.label}
				value={form?.expiration?.value}
				valid={form?.expiration?.valid}
				config={CARD_FIELDS.expiration}
				handleField={handleField}
			/>
			<Field
				params={props.params}
				kind={FieldKind.INPUT}
				fieldKey="cvc"
				label={CARD_FIELDS.cvc?.label}
				value={form?.cvc?.value}
				valid={form?.cvc?.valid}
				config={CARD_FIELDS.cvc}
				handleField={handleField}
			/>
			<Show when={formValid()}>
				<Field
					kind={FieldKind.CHECKBOX}
					fieldKey="consent"
					value={form?.consent?.value}
					valid={form?.consent?.valid}
					config={CARD_FIELDS.consent}
					handleField={handleField}
				/>
			</Show>
			<button
				class="button is-primary is-fullwidth"
				type="submit"
				disabled={!isSendable()}
				onClick={(e) => {
					e.preventDefault();
					sendForm();
				}}
			>
				Let's Go!
			</button>
		</form>
	);
};

export interface CardForm {
	name: {
		value: string;
		valid: null | boolean;
	};
	number: {
		value: string;
		valid: null | boolean;
	};
	expiration: {
		value: string;
		valid: null | boolean;
	};
	cvc: {
		value: string;
		valid: null | boolean;
	};
	consent: {
		value: boolean;
		valid: null | boolean;
	};
}

export const initCardFrom = () => {
	return {
		name: {
			value: "",
			valid: null,
		},
		number: {
			value: "",
			valid: null,
		},
		expiration: {
			value: "",
			valid: null,
		},
		cvc: {
			value: "",
			valid: null,
		},
		consent: {
			value: false,
			valid: null,
		},
	} as CardForm;
};

const CARD_FIELDS = {
	name: {
		label: "Name on Card",
		type: "text",
		placeholder: "Full Name",
		icon: "fas fa-signature",
		help: "Must be a non-empty string",
		validate: validNonEmpty,
	},
	number: {
		label: "Card Number",
		type: "text",
		placeholder: "3530-1113-3330-0000",
		icon: "fas fa-credit-card",
		help: "May only use numbers with optional spaces and dashes",
		validate: validCardNumber,
	},
	expiration: {
		label: "Expiration",
		type: "text",
		placeholder: "MM/YY",
		icon: "far fa-calendar-check",
		help: "Must be a valid current or future month / year",
		validate: validExpiration,
	},
	cvc: {
		label: "CVC",
		type: "text",
		placeholder: "123",
		icon: "fas fa-undo",
		help: "May only use three or four numbers",
		validate: validCardCvc,
	},
	consent: {
		label: "I Agree",
		type: "checkbox",
		placeholder: (
			<small>
				{" "}
				I agree to the{" "}
				{
					<a href="/legal/subscription" target="_blank" rel="noreferrer">
						subscription agreement
					</a>
				}
				.
			</small>
		),
	},
};

export default PaymentCard;
