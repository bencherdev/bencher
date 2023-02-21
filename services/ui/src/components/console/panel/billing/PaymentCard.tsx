import { createSignal } from "solid-js";
import Field from "../../../field/Field";
import FieldKind from "../../../field/kind";
import { is_valid_email, is_valid_user_name } from "bencher_valid";
import {
	validate_card_number,
	validate_expiration,
	validate_string,
} from "../../../site/util";

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
			<Field
				kind={FieldKind.INPUT}
				fieldKey="expiration"
				label={true}
				value={form()?.expiration?.value}
				valid={form()?.expiration?.valid}
				config={CARD_FIELDS.expiration}
				handleField={handleField}
			/>
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
		help: "May only use numbers and dashes",
		validate: validate_card_number,
	},
	expiration: {
		label: "Expiration",
		type: "text",
		placeholder: "MM/YY",
		icon: "far fa-calendar-check",
		help: "Must be a valid future month / year",
		validate: validate_expiration,
	},
};

export default PaymentCard;
