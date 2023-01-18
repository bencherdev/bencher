import axios from "axios";
import {
	createMemo,
	createResource,
	createSignal,
	For,
	Match,
	Switch,
} from "solid-js";
import Field from "../../../field/Field";
import { post_options, validate_jwt } from "../../../site/util";
import { useLocation, useNavigate } from "solid-app-router";
import FieldKind from "../../../field/kind";

const initForm = (fields) => {
	let newForm = {};
	fields.forEach((field) => {
		if (field.key) {
			newForm[field.key] = {};
			newForm[field.key].kind = field.kind;
			newForm[field.key].label = field.label;
			newForm[field.key].value = field.value;
			newForm[field.key].valid = field.valid;
			newForm[field.key].validate = field.validate;
			newForm[field.key].nullable = field.nullable;
		}
	});
	newForm.submitting = false;
	return newForm;
};

const Poster = (props) => {
	const navigate = useNavigate();
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);

	const [form, setForm] = createSignal(initForm(props.config?.fields));
	const [valid, setValid] = createSignal(false);

	// setInterval(() => console.log(form()), 3000);

	const is_sendable = (): boolean => {
		return !form()?.submitting && valid();
	};

	const post = async (data: {}) => {
		try {
			const token = props.user()?.token;
			if (!validate_jwt(props.user()?.token)) {
				return;
			}

			const url = props.config?.url?.(props.path_params());
			await axios(post_options(url, token, data));
			navigate(props.config?.path?.(pathname()));
		} catch (error) {
			console.error(error);
		}
	};

	function sendForm(e) {
		e.preventDefault();
		if (!is_sendable()) {
			return;
		}

		handleFormSubmitting(true);
		let data = {};
		for (let key of Object.keys(form())) {
			const value = form()?.[key]?.value;
			switch (form()?.[key]?.kind) {
				case FieldKind.SELECT:
					if (form()?.[key]?.nullable && !value?.selected) {
						continue;
					}
					data[key] = value?.selected;
					break;
				case FieldKind.NUMBER:
					if (form()?.[key]?.nullable && !value) {
						continue;
					}
					data[key] = Number(value);
					break;
				default:
					if (form()?.[key]?.nullable && !value) {
						continue;
					}
					if (typeof value === "string") {
						data[key] = value.trim();
					} else {
						data[key] = value;
					}
			}
		}
		delete data.submitting;
		post(data).then(() => handleFormSubmitting(false));
	}

	const handleFormSubmitting = (submitting) => {
		setForm({ ...form(), submitting: submitting });
	};

	const handleField = (key, value, valid) => {
		if (key && form()?.[key]) {
			if (form()?.[key]?.nullable && !value) {
				value = null;
				valid = true;
			}

			setForm({
				...form(),
				[key]: {
					...form()?.[key],
					value: value,
					valid: valid,
				},
			});
			setValid(isValid());
		}
	};

	function isValid() {
		const form_values = Object.values(form());
		for (let i = 0; i < form_values.length; i++) {
			if (form_values[i]?.validate && !form_values[i]?.valid) {
				return false;
			}
		}
		return true;
	}

	return (
		<div class="columns">
			<div class="column">
				<form class="box">
					<For each={props.config?.fields}>
						{(field, i) => (
							<Field
								key={i}
								kind={field?.kind}
								label={form()?.[field?.key]?.label}
								fieldKey={field?.key}
								value={form()?.[field?.key]?.value}
								valid={form()?.[field?.key]?.valid}
								config={field?.config}
								user={props.user}
								path_params={props.path_params}
								handleField={handleField}
							/>
						)}
					</For>
					<br />
					<button
						class="button is-primary is-fullwidth"
						disabled={!is_sendable()}
						onClick={sendForm}
					>
						Save
					</button>
				</form>
			</div>
		</div>
	);
};

export default Poster;
