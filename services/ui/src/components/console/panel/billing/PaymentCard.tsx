import { createSignal } from "solid-js";
import Field from "../../../field/Field";
import FieldKind from "../../../field/kind";
import Pricing, { Plan } from "./Pricing";
import { is_valid_email, is_valid_user_name } from "bencher_valid";
import { validate_string } from "../../../site/util";

const PaymentCard = (props) => {
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
		return validate_form.number.valid; //&&
		// validate_form.card_exp_month.valid &&
		// validate_form.card_exp_year.valid &&
		// validate_form.card_cvc.valid
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

	return (
		<form class="box">
			<Field
				kind={FieldKind.INPUT}
				fieldKey="number"
				label={true}
				value={form()?.number?.value}
				valid={form()?.number?.valid}
				config={CARD_FIELDS.number}
				handleField={handleField}
			/>
			{/* <Field
				kind={FieldKind.INPUT}
				fieldKey="email"
				label={true}
				value={form()?.email?.value}
				valid={form()?.email?.valid}
				config={CARD_FIELDS.email}
				handleField={handleField}
			/> */}
		</form>
	);
};

export const cardForm = () => {
	return {
		number: {
			value: "",
			valid: null,
		},
		exp_month: {
			value: "",
			valid: null,
		},
		exp_year: {
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
		placeholder: "4000-0082-6000-0000",
		icon: "fas fa-credit-card",
		help: "May only use: letters, numbers, contained spaces, apostrophes, periods, commas, and dashes",
		validate: (input) => validate_string(input, is_valid_user_name),
	},
	// email: {
	// 	label: "Email",
	// 	type: "email",
	// 	placeholder: "email@example.com",
	// 	icon: "fas fa-envelope",
	// 	help: "Must be a valid email address",
	// 	validate: (input) => validate_string(input, is_valid_email),
	// },
};

export default PaymentCard;
