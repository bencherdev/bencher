import { InitOutput } from "bencher_valid";
import { For, Resource, Show, createSignal } from "solid-js";
import Field, { FieldConfig, FieldValue } from "../../field/Field";
import FieldKind from "../../field/kind";
import { createStore } from "solid-js/store";
import { authUser } from "../../../util/auth";
import { Params, pathname, useNavigate, useParams } from "../../../util/url";
import { validJwt } from "../../../util/valid";
import { Operation } from "../../../config/types";
import { httpPost, httpPut } from "../../../util/http";

export interface Props {
	path: string;
	operation: Operation;
	config: PosterConfig;
	bencher_valid: Resource<InitOutput>;
}

export interface PosterConfig {
	url: (pathParams: Params) => string;
	fields: PosterFieldConfig[];
	path: (pathname: string) => string;
	button: string;
}

export interface PosterFieldConfig {
	kind: FieldKind;
	label: string;
	key: string;
	value: FieldValue;
	valid: null | boolean;
	validate: boolean;
	config: FieldConfig;
	nullable?: null | boolean;
}

type PosterForm = Record<string, PosterField>;

interface PosterField {
	kind: FieldKind;
	label: string;
	value: FieldValue;
	valid: null | boolean;
	validate: boolean;
	nullable: undefined | null | boolean;
}

const initForm = (fields: PosterFieldConfig[]) => {
	let newForm: PosterForm = {};
	fields.forEach((field) => {
		if (field.key) {
			newForm[field.key] = {
				kind: field.kind,
				label: field.label,
				value: field.value,
				valid: field.valid,
				validate: field.validate,
				nullable: field.nullable,
			};
		}
	});
	return newForm;
};

const Poster = (props: Props) => {
	const navigate = useNavigate();

	const user = authUser();
	const pathParams = useParams(props.path);
	const [form, setForm] = createStore(initForm(props.config?.fields));
	const [submitting, setSubmitting] = createSignal(false);
	const [valid, setValid] = createSignal(false);

	const isSendable = (): boolean => {
		return !submitting() && valid();
	};

	const httpOperation = async (
		url: string,
		token: string,
		data: Record<string, undefined | number | string>,
	) => {
		switch (props.operation) {
			case Operation.EDIT:
				return await httpPut(url, token, data);
			case Operation.ADD:
			default:
				return await httpPost(url, token, data);
		}
	};

	function sendForm() {
		if (!props.bencher_valid()) {
			return;
		}
		const token = user?.token;
		if (!validJwt(token)) {
			return;
		}
		if (!isSendable()) {
			return;
		}

		setSubmitting(true);
		let data: Record<string, undefined | number | string> = {};
		for (let key of Object.keys(form)) {
			const value = form?.[key]?.value;
			switch (form?.[key]?.kind) {
				case FieldKind.SELECT:
					if (form?.[key]?.nullable && !value?.selected) {
						continue;
					}
					data[key] = value?.selected;
					break;
				case FieldKind.NUMBER:
					if (form?.[key]?.nullable && !value) {
						continue;
					}
					data[key] = Number(value);
					break;
				default:
					if (form?.[key]?.nullable && !value) {
						continue;
					}
					if (typeof value === "string") {
						data[key] = value.trim();
					} else {
						data[key] = value;
					}
			}
		}

		const url = props.config?.url?.(pathParams);
		httpOperation(url, token, data)
			.then((_resp) => {
				setSubmitting(false);
				navigate(props.config?.path?.(pathname()));
				// navigate(
				// 	notification_path(
				// 		props.config?.path?.(pathname()),
				// 		[],
				// 		[],
				// 		NotifyKind.OK,
				// 		"Creation successful!",
				// 	),
				// );
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				// navigate(
				// 	notification_path(
				// 		pathname(),
				// 		[],
				// 		[],
				// 		NotifyKind.ERROR,
				// 		"Failed to create. Please, try again.",
				// 	),
				// );
			});
	}

	const handleField = (key: string, value: FieldValue, valid: boolean) => {
		if (key && form?.[key]) {
			if (form?.[key]?.nullable && !value) {
				value = null;
				valid = true;
			}

			setForm({
				...form,
				[key]: {
					...form?.[key],
					value: value,
					valid: valid,
				},
			});
			setValid(isValid());
		}
	};

	function isValid() {
		const form_values = Object.values(form);
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
						{(field, _i) => (
							<Field
								user={user}
								kind={field?.kind}
								label={form?.[field?.key]?.label}
								fieldKey={field?.key}
								value={form?.[field?.key]?.value}
								valid={form?.[field?.key]?.valid}
								config={field?.config}
								pathParams={pathParams}
								handleField={handleField}
							/>
						)}
					</For>
					<br />
					<div class="field">
						<p class="control">
							<button
								class="button is-primary is-fullwidth"
								disabled={!isSendable()}
								onClick={(e) => {
									e.preventDefault();
									sendForm();
								}}
							>
								<Show when={props.config?.button} fallback={"Save"}>
									{props.config?.button}
								</Show>
							</button>
						</p>
					</div>
				</form>
			</div>
		</div>
	);
};

export default Poster;
