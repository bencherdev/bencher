import { createSignal } from "solid-js";
import Field from "../../../field/Field";
import FieldKind from "../../../field/kind";
import { is_valid_email, is_valid_user_name } from "bencher_valid";
import {
	validate_card_cvc,
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
			<Field
				kind={FieldKind.INPUT}
				fieldKey="cvc"
				label={true}
				value={form()?.cvc?.value}
				valid={form()?.cvc?.valid}
				config={CARD_FIELDS.cvc}
				handleField={handleField}
			/>
			<button
				class="button is-primary is-fullwidth"
				disabled={!form()?.valid || form()?.submitting}
				// onClick={handleFormSubmit}
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
		help: "May only use numbers and optional dashes",
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
