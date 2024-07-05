import { createSignal, type Accessor } from "solid-js";
import type { JsonAuthUser } from "../../../../../types/bencher";
import type CardConfig from "./CardConfig";
import type { PosterFieldConfig } from "../../../poster/Poster";
import FieldKind from "../../../../field/kind";
import type { FieldValue } from "../../../../field/Field";
import Field from "../../../../field/Field";
import { createStore } from "solid-js/store";
import { validJwt } from "../../../../../util/valid";
import { httpPatch } from "../../../../../util/http";
import type { Params } from "astro";
import {
	NotifyKind,
	navigateNotify,
	pageNotify,
} from "../../../../../util/notify";

export interface Props {
	apiUrl: string;
	params: Params;
	user: JsonAuthUser;
	path: Accessor<string>;
	card: CardConfig;
	value: boolean | string;
	toggleUpdate: () => void;
	handleRefresh: () => void;
	handleLoopback: (pathname: null | string) => void;
}

const initForm = (field: PosterFieldConfig, value: FieldValue) => {
	switch (field?.kind) {
		case FieldKind.SELECT:
			field.value.selected = value;
			break;
		default:
			field.value = value;
	}

	return {
		[field?.key]: {
			kind: field?.kind,
			label: field?.label,
			value: field?.value,
			valid: field?.valid,
			validate: field?.validate,
			nullable: field?.nullable,
		},
	};
};

const UpdateCard = (props: Props) => {
	const [form, setForm] = createStore(initForm(props.card?.field, props.value));
	const [valid, setValid] = createSignal(false);
	const [submitting, setSubmitting] = createSignal(false);

	const isSendable = (): boolean => {
		return valid() && !isUnchanged() && !submitting();
	};

	const isUnchanged = () => {
		switch (props.card?.field?.kind) {
			case FieldKind.SELECT:
				return props.value === form?.[props.card?.field?.key]?.value?.selected;
			default:
				return props.value === form?.[props.card?.field?.key]?.value;
		}
	};

	const sendForm = () => {
		if (!isSendable()) {
			return;
		}
		const token = props.user?.token;
		if (!validJwt(token)) {
			return;
		}

		setSubmitting(true);
		const data = {};
		for (const key of Object.keys(form)) {
			const value = form?.[key]?.value;
			switch (form?.[key]?.kind) {
				case FieldKind.SELECT:
					if (form?.[key]?.nullable && !value?.selected) {
						continue;
					}
					data[key] = value?.selected;
					break;
				case FieldKind.NUMBER:
					if (form?.[key]?.nullable && !value && value !== null) {
						continue;
					}
					data[key] = Number(value);
					break;
				default:
					if (form?.[key]?.nullable && !value && value !== null) {
						continue;
					}
					if (typeof value === "string") {
						data[key] = value.trim();
					} else {
						data[key] = value;
					}
			}
		}

		httpPatch(props.apiUrl, props.path(), token, data)
			.then((_resp) => {
				setSubmitting(false);
				const path = updatePath(data);
				const text = `Hare's to your success! You've updated ${props.card?.field?.label?.toLowerCase()}.`;
				if (path) {
					navigateNotify(NotifyKind.OK, text, path, null, null);
				} else {
					props.toggleUpdate();
					props.handleRefresh();
					if (props.card?.notify ?? true) {
						pageNotify(NotifyKind.OK, text);
					}
				}
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				pageNotify(
					NotifyKind.ERROR,
					`Lettuce romaine calm! Failed to update ${props.card?.field?.label?.toLowerCase()}. Please, try again.`,
				);
			});
	};

	const updatePath = (data: object) => {
		// TODO once the above navigation moves over to a soft redirect
		// Then these refreshes can be reenabled as opposed to a hard page reload
		if (props.card?.path) {
			const path = props.card?.path(props.params, data);
			// props.handleLoopback(path);
			return path;
		}
		return;
	};

	const handleField = (key: string, value: FieldValue, valid: boolean) => {
		if (key && form?.[key]) {
			if (form?.[key]?.nullable && !value) {
				// biome-ignore lint/style/noParameterAssign: TODO
				value = null;
				// biome-ignore lint/style/noParameterAssign: TODO
				valid = true;
			}

			setForm({
				...form,
				[key]: {
					...form?.[key],
					value,
					valid,
				},
			});

			setValid(isValid());
		}
	};

	const isValid = () => {
		const form_values = Object.values(form);
		for (let i = 0; i < form_values.length; i++) {
			if (form_values[i]?.validate && !form_values[i]?.valid) {
				return false;
			}
		}
		return true;
	};

	return (
		<form
			onSubmit={(e) => {
				e.preventDefault();
				sendForm();
			}}
		>
			<div id={props.card?.label} class="field is-horizontal">
				<div class="field-label is-normal">
					<label class="label">{props.card?.label}</label>
				</div>
				<div class="field-body">
					<Field
						params={props.params}
						kind={props.card?.field?.kind}
						fieldKey={props.card?.field?.key}
						value={form?.[props.card?.field?.key]?.value}
						valid={form?.[props.card?.field?.key]?.valid}
						config={props.card?.field?.config}
						handleField={handleField}
					/>
					<div class="field">
						<div class="control">
							<div class="buttons">
								<button
									type="button"
									class="button"
									disabled={!isSendable()}
									onMouseDown={(e) => {
										e.preventDefault();
										sendForm();
									}}
								>
									Save
								</button>
								<button
									type="button"
									class="button"
									onMouseDown={(e) => {
										e.preventDefault();
										props.toggleUpdate();
									}}
								>
									Cancel
								</button>
							</div>
						</div>
					</div>
				</div>
			</div>
		</form>
	);
};

export default UpdateCard;
