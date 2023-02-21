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
			<div class="field is-horizontal">
				<div class="field-label is-normal">
					<label class="label">From</label>
				</div>
				<div class="field-body">
					<div class="field">
						<p class="control is-expanded has-icons-left">
							<input class="input" type="text" placeholder="Name" />
							<span class="icon is-small is-left">
								<i class="fas fa-user"></i>
							</span>
						</p>
					</div>
					<div class="field">
						<p class="control is-expanded has-icons-left has-icons-right">
							<input
								class="input is-success"
								type="email"
								placeholder="Email"
								value="alex@smith.com"
							/>
							<span class="icon is-small is-left">
								<i class="fas fa-envelope"></i>
							</span>
							<span class="icon is-small is-right">
								<i class="fas fa-check"></i>
							</span>
						</p>
					</div>
				</div>
			</div>

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
		help: "May only use numbers: NO DASHES",
		validate: validate_card_number,
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
