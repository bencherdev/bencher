import * as Sentry from "@sentry/astro";
import { type Resource, createSignal } from "solid-js";
import type { JsonAuthUser, JsonProject } from "../../../types/bencher";
import { httpPost } from "../../../util/http";
import { NotifyKind, navigateNotify } from "../../../util/notify";
import { useNavigate } from "../../../util/url";
import { CLAIM_PARAM } from "../../auth/auth";

interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	project: Resource<JsonProject>;
}

const ClaimBanner = (props: Props) => {
	const [claiming, setClaiming] = createSignal(false);

	const claimProject = async () => {
		const token = props.user?.token;
		const organization_uuid = props.project()?.organization;
		if (!props.apiUrl || !token) {
			if (organization_uuid) {
				const searchParams = new URLSearchParams();
				searchParams.set(CLAIM_PARAM, organization_uuid);
				const navigate = useNavigate();
				navigate(`/auth/signup?${searchParams.toString()}`);
			}
			return null;
		}
		setClaiming(true);
		const path = `/v0/organizations/${organization_uuid}/claim`;
		return await httpPost(props.apiUrl, path, token, {})
			.then((_resp) => {
				navigateNotify(
					NotifyKind.OK,
					"Project claimed!",
					"/console",
					null,
					null,
				);
			})
			.catch((error) => {
				setClaiming(false);
				console.error(error);
				Sentry.captureException(error);
				navigateNotify(
					NotifyKind.ERROR,
					`Invalid claim attempt: ${error}`,
					null,
					null,
					null,
				);
			});
	};

	return (
		<div class="content has-text-centered" style="margin-top: 1rem;">
			<div class="columns is-centered">
				<div class="column">
					<button
						type="button"
						class="button is-primary is-fullwidth"
						title={
							claiming()
								? `Claiming ${props.project()?.name ?? "project"}...`
								: `Claim ${props.project()?.name ?? "this project"}`
						}
						disabled={claiming()}
						onMouseDown={async (e) => {
							e.preventDefault();
							await claimProject();
						}}
					>
						🐰 {props.project()?.name ?? "This project"} is unclaimed!
						<br />
						Claim this project
					</button>
				</div>
			</div>
		</div>
	);
};

export default ClaimBanner;
