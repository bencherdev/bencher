import { createEffect, createMemo, createSignal } from "solid-js";
import Field from "../../../field/Field";
import FieldKind from "../../../field/kind";
import { is_valid_email, is_valid_user_name } from "bencher_valid";
import {
	BENCHER_API_URL,
	post_options,
	validate_card_cvc,
	validate_card_number,
	validate_expiration,
	validate_string,
	PLAN_PARAM,
	validate_jwt,
	NotifyKind,
	clean_card_number,
	clean_expiration,
} from "../../../site/util";
import axios from "axios";
import { notification_path } from "../../../site/Notification";
import { useLocation, useNavigate } from "solid-app-router";
import { PlanLevel } from "./Pricing";

const PaymentCard = (props) => {
	const navigate = useNavigate();
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);

	const form = createMemo(() => props.form());
	const handleField = (key, value, valid) => {
		props.handleForm({
			...form(),
			[key]: {
				value: value,
				valid: valid,
			},
		});
	};

	const validateForm = () => {
		const validate_form = form();
		return (
			validate_form.number.valid &&
			validate_form.expiration.valid &&
			validate_form.cvc.valid
		);
	};

	const handleFormValid = () => {
		var valid = validateForm();
		if (valid !== form()?.valid) {
			props.handleForm({ ...form(), valid: valid });
		}
	};

	const handleFormSubmitting = (submitting) => {
		props.handleForm({ ...form(), submitting: submitting });
	};

	const post = async (data: {
		card: {
			number: string;
			exp_month: number;
			exp_year: number;
			cvc: string;
		};
		level: PlanLevel;
	}) => {
		const token = props.user?.token;
		if (!validate_jwt(props.user?.token)) {
			return;
		}
		return await axios(post_options(props.url, token, data))
			.then((resp) => resp?.data)
			.catch(console.error);
	};

	const handleFormSubmit = (event) => {
		event.preventDefault();
		handleFormSubmitting(true);

		const number = clean_card_number(form()?.number?.value);
		const exp = clean_expiration(form()?.expiration?.value);
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

		post(data)
			.then((_data) => {
				handleFormSubmitting(false);
				props.handleRefresh();
				navigate(
					notification_path(
						pathname(),
						[],
						[],
						NotifyKind.OK,
						"Successful plan enrollment!",
					),
				);
			})
			.catch((_error) => {
				handleFormSubmitting(false);
				navigate(
					notification_path(
						pathname(),
						[PLAN_PARAM],
						[],
						NotifyKind.ERROR,
						"Failed to enroll, please try again.",
					),
				);
			});
	};

	createEffect(() => {
		handleFormValid();
	});

	return (
		<form class="box">
			<Field
				kind={FieldKind.INPUT}
				fieldKey="number"
				label={CARD_FIELDS.number?.label}
				value={form()?.number?.value}
				valid={form()?.number?.valid}
				config={CARD_FIELDS.number}
				handleField={handleField}
			/>
			<Field
				kind={FieldKind.INPUT}
				fieldKey="expiration"
				label={CARD_FIELDS.expiration?.label}
				value={form()?.expiration?.value}
				valid={form()?.expiration?.valid}
				config={CARD_FIELDS.expiration}
				handleField={handleField}
			/>
			<Field
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
				disabled={!form()?.valid || form()?.submitting}
				onClick={handleFormSubmit}
			>
				Let's Go!
			</button>
		</form>
	);
};

export const cardForm = () => {
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
		valid: false,
		submitting: false,
	};
};

const CARD_FIELDS = {
	number: {
		label: "Card Number",
		type: "text",
		placeholder: "3530-1113-3330-0000",
		icon: "fas fa-credit-card",
		help: "May only use numbers with optional spaces and dashes",
		validate: validate_card_number,
	},
	expiration: {
		label: "Expiration",
		type: "text",
		placeholder: "MM/YY",
		icon: "far fa-calendar-check",
		help: "Must be a valid current or future month / year",
		validate: validate_expiration,
	},
	cvc: {
		label: "CVC",
		type: "text",
		placeholder: "123",
		icon: "fas fa-undo",
		help: "May only use three or four numbers",
		validate: validate_card_cvc,
	},
};

export default PaymentCard;
