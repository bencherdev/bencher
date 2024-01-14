import type { Params } from "astro";
import type { InitOutput } from "bencher_valid";
import { type Accessor, type Resource, createSignal } from "solid-js";
import { createStore } from "solid-js/store";
import type {
	JsonAuthUser,
	JsonNewCheckout,
	PlanLevel,
} from "../../../../types/bencher";
import { httpPost } from "../../../../util/http";
import { NotifyKind, pageNotify } from "../../../../util/notify";
import { validJwt } from "../../../../util/valid";
import Field, { type FieldValue } from "../../../field/Field";
import FieldKind from "../../../field/kind";

import { useNavigate } from "../../../../util/url";

interface Props {
	apiUrl: string;
	params: Params;
	bencher_valid: Resource<InitOutput>;
	user: JsonAuthUser;
	organization: undefined | string;
	plan: Accessor<PlanLevel>;
	entitlements: Accessor<null | number>;
	organizationUuid: Accessor<null | string>;
	organizationUuidValid: Accessor<boolean>;
	handleRefresh: () => void;
}

const Checkout = (props: Props) => {
	const [form, setForm] = createStore(initCardFrom());
	const [submitting, setSubmitting] = createSignal(false);
	const navigate = useNavigate();

	const isSendable = (): boolean => {
		return (
			!submitting() &&
			(form?.consent?.valid ?? false) &&
			props.organizationUuidValid()
		);
	};

	const handleField = (key: string, value: FieldValue, valid: boolean) => {
		setForm({
			...form,
			[key]: {
				value: value,
				valid: valid,
			},
		});
	};

	const sendForm = () => {
		if (!props.bencher_valid()) {
			return;
		}
		const token = props.user?.token;
		if (!validJwt(token)) {
			return;
		}
		if (!isSendable()) {
			return;
		}

		const newCheckout: JsonNewCheckout = {
			organization: props.organization,
			level: props.plan(),
			entitlements: props.entitlements(),
			self_hosted_organization: props.organizationUuid(),
			i_agree: form?.consent?.value,
		};

		setSubmitting(true);
		httpPost(props.apiUrl, "/v0/checkout", token, newCheckout)
			.then((checkout) => {
				console.log(checkout.data);
				navigate(checkout.data.url);
				setSubmitting(false);
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				pageNotify(
					NotifyKind.ERROR,
					"Lettuce romaine calm! Failed to enroll. Please, try again.",
				);
			});
	};

	return (
		<form class="box">
			<Field
				kind={FieldKind.CHECKBOX}
				fieldKey="consent"
				value={form?.consent?.value}
				valid={form?.consent?.valid}
				config={CARD_FIELDS.consent}
				handleField={handleField}
			/>
			<button
				class="button is-primary is-fullwidth"
				disabled={!isSendable()}
				onClick={(e) => {
					e.preventDefault();
					sendForm();
				}}
			>
				Let's Go!
			</button>
		</form>
	);
};

export interface CardForm {
	consent: {
		value: boolean;
		valid: null | boolean;
	};
}

export const initCardFrom = () => {
	return {
		consent: {
			value: false,
			valid: null,
		},
	} as CardForm;
};

const CARD_FIELDS = {
	consent: {
		label: "I Agree",
		type: "checkbox",
		placeholder: (
			<small>
				{" "}
				I agree to the{" "}
				{
					<a href="/legal/subscription" target="_blank">
						subscription agreement
					</a>
				}
				.
			</small>
		),
	},
};

export default Checkout;
