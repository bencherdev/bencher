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
} from "../../../site/util";
import axios from "axios";
import { notification_path } from "../../../site/Notification";
import { useLocation, useNavigate } from "solid-app-router";

const PaymentCard = (props) => {
	const navigate = useNavigate();
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);

	const [form, setForm] = createSignal(cardForm());
	const handleField = (key, value, valid) => {
		setForm({
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
			setForm({ ...form(), valid: valid });
		}
	};

	const handleFormSubmitting = (submitting) => {
		setForm({ ...form(), submitting: submitting });
	};

	const post = async (data: {
		name: null | string;
		slug: null | string;
		email: string;
		invite: null | string;
	}) => {
		const url = `${BENCHER_API_URL()}/v0/organizations/${props.config?.kind}`;
		const no_token = null;
		let resp = await axios(post_options(url, no_token, data));
		return resp.data;
	};

	const handleFormSubmit = (event) => {
		event.preventDefault();
		handleFormSubmitting(true);
		const invite_token = props.invite();
		let invite: string | null;
		if (validate_jwt(invite_token)) {
			invite = invite_token;
		} else {
			invite = null;
		}

		const data = {};

		post(data)
			.then((_resp) => {
				navigate(
					notification_path(
						props.config?.redirect,
						[],
						[],
						NotifyKind.OK,
						"Successful plan enrollment!",
					),
				);
			})
			.catch((_e) => {
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

		handleFormSubmitting(false);
	};

	createEffect(() => {
		handleFormValid();
	});

	return (
		<form class="box">
			<Field
				kind={FieldKind.INPUT}
				fieldKey="number"
				label={CARD_FIELDS.number.label}
				value={form()?.number?.value}
				valid={form()?.number?.valid}
				config={CARD_FIELDS.number}
				handleField={handleField}
			/>
			<Field
				kind={FieldKind.INPUT}
				fieldKey="expiration"
				label={CARD_FIELDS.expiration.label}
				value={form()?.expiration?.value}
				valid={form()?.expiration?.valid}
				config={CARD_FIELDS.expiration}
				handleField={handleField}
			/>
			<Field
				kind={FieldKind.INPUT}
				fieldKey="cvc"
				label={CARD_FIELDS.cvc.label}
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
