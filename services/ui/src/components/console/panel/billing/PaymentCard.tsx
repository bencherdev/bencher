import { createSignal } from "solid-js";
import Field from "../../../field/Field";
import FieldKind from "../../../field/kind";
import { is_valid_email, is_valid_user_name } from "bencher_valid";
import { validate_card_number, validate_string } from "../../../site/util";

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
				fieldKey="expiration"
				label={true}
				value={form()?.number?.value}
				valid={form()?.number?.valid}
				config={CARD_FIELDS.expiration}
				handleField={handleField}
			/> */}

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
		placeholder: "3530-1113-3330-0000",
		icon: "fas fa-credit-card",
		help: "May only use numbers and dashes",
		validate: validate_card_number,
	},
	expiration: {
		label: "Expiration",
		type: "email",
		placeholder: "MM/YY",
		icon: "fas fa-envelope",
		help: "Must be a valid month and year",
		validate: (input) => validate_string(input, is_valid_email),
	},
};

export default PaymentCard;
