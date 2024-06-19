import type { Params } from "astro";
import type { InitOutput } from "bencher_valid";
import { type Accessor, type Resource, createSignal } from "solid-js";
import type {
	JsonAuthUser,
	JsonNewCheckout,
	PlanLevel,
} from "../../../../types/bencher";
import { httpPost } from "../../../../util/http";
import { NotifyKind, pageNotify } from "../../../../util/notify";
import { useNavigate } from "../../../../util/url";
import { validJwt } from "../../../../util/valid";

interface Props {
	apiUrl: string;
	params: Params;
	onboard: boolean;
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
	const [submitting, setSubmitting] = createSignal(false);
	const navigate = useNavigate();

	const isSendable = (): boolean => {
		return !submitting() && props.organizationUuidValid();
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
			self_hosted: props.organizationUuid(),
		};

		setSubmitting(true);
		httpPost(props.apiUrl, "/v0/checkout", token, newCheckout)
			.then((checkout) => {
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
		<div class="columns is-centered" style="margin-top: 1rem;">
			<div class={`column ${props.onboard ? "" : "is-half"}`}>
				<button
					class="button is-primary is-fullwidth"
					type="submit"
					disabled={!isSendable()}
					onMouseDown={(e) => {
						e.preventDefault();
						sendForm();
					}}
				>
					<span class="icon-text">
						<span>Activate</span>
						<span class="icon">
							<i class="fas fa-chevron-right" />
						</span>
					</span>
				</button>
			</div>
		</div>
	);
};

export default Checkout;
