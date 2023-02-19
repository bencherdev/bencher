import { createSignal } from "solid-js";
import Field from "../../../field/Field";
import FieldKind from "../../../field/kind";
import Pricing, { Plan } from "./Pricing";
import { is_valid_email, is_valid_user_name } from "bencher_valid";
import { validate_string } from "../../../site/util";

const PaymentCard = (props) => {
	const [form, setForm] = createSignal(initForm());
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
			validate_form.name.valid &&
			validate_form.card_number.valid &&
			validate_form.card_exp_month.valid &&
			validate_form.card_exp_year.valid &&
			validate_form.card_cvc.valid
		);
	};

	return (
		<form class="box">
			<Field
				kind={FieldKind.INPUT}
				fieldKey="name"
				label={true}
				value={form()?.name?.value}
				valid={form()?.name?.valid}
				config={CARD_FIELDS.name}
				handleField={handleField}
			/>
			<Field
				kind={FieldKind.INPUT}
				fieldKey="email"
				label={true}
				value={form()?.email?.value}
				valid={form()?.email?.valid}
				config={CARD_FIELDS.email}
				handleField={handleField}
			/>
		</form>
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
		card_number: {
			value: "",
			valid: null,
		},
		card_exp_month: {
			value: "",
			valid: null,
		},
		card_exp_year: {
			value: "",
			valid: null,
		},
		card_cvc: {
			value: "",
			valid: null,
		},
		valid: false,
		submitting: false,
	};
};

const CARD_FIELDS = {
	name: {
		label: "Name",
		type: "text",
		placeholder: "Full Name",
		icon: "fas fa-user",
		help: "May only use: letters, numbers, contained spaces, apostrophes, periods, commas, and dashes",
		validate: (input) => validate_string(input, is_valid_user_name),
	},
	email: {
		label: "Email",
		type: "email",
		placeholder: "email@example.com",
		icon: "fas fa-envelope",
		help: "Must be a valid email address",
		validate: (input) => validate_string(input, is_valid_email),
	},
};

export default PaymentCard;
