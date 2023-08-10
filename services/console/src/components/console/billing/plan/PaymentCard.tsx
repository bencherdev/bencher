import type { InitOutput } from "bencher_valid";
import { createSignal, type Accessor, Resource, createEffect } from "solid-js";
import type { JsonAuthUser, PlanLevel } from "../../../../types/bencher";
import {
	cleanCardNumber,
	cleanExpiration,
	validCardCvc,
	validCardNumber,
	validExpiration,
	validJwt,
} from "../../../../util/valid";
import Field, { FieldValue } from "../../../field/Field";
import FieldKind from "../../../field/kind";
import { httpPost } from "../../../../util/http";
import type { Params } from "astro";

interface Props {
	params: Params;
	bencher_valid: Resource<InitOutput>;
	user: JsonAuthUser;
	url: string;
	plan: Accessor<PlanLevel>;
	handleRefresh: () => void;
}

const PaymentCard = (props: Props) => {
	const [form, setForm] = createSignal(initCardFrom());
	const [submitting, setSubmitting] = createSignal(false);
	const [valid, setValid] = createSignal(false);

	const isSendable = (): boolean => {
		return !submitting() && valid();
	};

	const handleField = (key: string, value: FieldValue, valid: boolean) => {
		setForm({
			...form(),
			[key]: {
				value: value,
				valid: valid,
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

		const number = cleanCardNumber(form()?.number?.value);
		const exp = cleanExpiration(form()?.expiration?.value);
		if (exp === null) {
			return;
		}
		const cvc = form()?.cvc?.value?.trim();
		const card = {
			number: number,
			exp_month: exp[0],
			exp_year: exp[1],
			cvc: cvc,
		};
		const data = {
			card: card,
			level: props.plan(),
		};

		setSubmitting(true);
		httpPost(props.url, token, data)
			.then((_resp) => {
				setSubmitting(false);
				props.handleRefresh();
				// navigate(
				// 	notification_path(
				// 		pathname(),
				// 		[],
				// 		[],
				// 		NotifyKind.OK,
				// 		"Successful plan enrollment!",
				// 	),
				// );
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				// navigate(
				// 	notification_path(
				// 		pathname(),
				// 		[PLAN_PARAM],
				// 		[],
				// 		NotifyKind.ERROR,
				// 		"Failed to enroll. Please, try again.",
				// 	),
				// );
			});
	};

	createEffect(() => {
		const f = form();
		if (!valid() && f.number.valid && f.expiration.valid && f.cvc.valid) {
			setValid(true);
		}
	});

	return (
		<form class="box">
			<Field
				params={props.params}
				user={props.user}
				kind={FieldKind.INPUT}
				fieldKey="number"
				label={CARD_FIELDS.number?.label}
				value={form()?.number?.value}
				valid={form()?.number?.valid}
				config={CARD_FIELDS.number}
				handleField={handleField}
			/>
			<Field
				params={props.params}
				user={props.user}
				kind={FieldKind.INPUT}
				fieldKey="expiration"
				label={CARD_FIELDS.expiration?.label}
				value={form()?.expiration?.value}
				valid={form()?.expiration?.valid}
				config={CARD_FIELDS.expiration}
				handleField={handleField}
			/>
			<Field
				params={props.params}
				user={props.user}
				kind={FieldKind.INPUT}
				fieldKey="cvc"
				label={CARD_FIELDS.cvc?.label}
				value={form()?.cvc?.value}
				valid={form()?.cvc?.valid}
				config={CARD_FIELDS.cvc}
				handleField={handleField}
			/>
			<br />
			<button
				class="button is-primary is-fullwidth"
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
}

export const initCardFrom = () => {
	return {
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
	} as CardForm;
};

const CARD_FIELDS = {
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
};

export default PaymentCard;
