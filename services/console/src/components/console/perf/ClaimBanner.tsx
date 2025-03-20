import * as Sentry from "@sentry/astro";
import {
	type Accessor,
	type Resource,
	createMemo,
	createSignal,
} from "solid-js";
import type { JsonAuthUser, JsonProject } from "../../../types/bencher";
import { httpPost } from "../../../util/http";
import { NotifyKind, navigateNotify } from "../../../util/notify";
import { useNavigate } from "../../../util/url";
import { CLAIM_PARAM } from "../../auth/auth";

interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	project: Resource<JsonProject>;
	project_slug: Accessor<string | undefined>;
}

const ClaimBanner = (props: Props) => {
	const [claiming, setClaiming] = createSignal(false);

	const claimProject = async () => {
		const token = props.user?.token;
		const project_slug = props.project_slug();
		if (!props.apiUrl || !token || !project_slug) {
			const organization_uuid = props.project()?.organization;
			if (organization_uuid) {
				const searchParams = new URLSearchParams();
				searchParams.set(CLAIM_PARAM, organization_uuid);
				const navigate = useNavigate();
				navigate(`/auth/signup?${searchParams.toString()}`);
			}
			return null;
		}
		setClaiming(true);
		const path = `/v0/projects/${project_slug}/claim`;
		return await httpPost(props.apiUrl, path, token, {})
			.then((resp) => {
				navigateNotify(
					NotifyKind.OK,
					"Invitation accepted!",
					"/auth/signup",
					null,
					[["invite", resp?.data?.invite]],
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
	const name = createMemo(() => props.project()?.name ?? "this project");

	return (
		<div class="content has-text-centered" style="margin-top: 1rem;">
			<div class="columns is-centered">
				<div class="column">
					<button
						type="button"
						class="button is-primary is-fullwidth"
						title={claiming() ? "Claiming project..." : `Claim ${name()}`}
						disabled={claiming()}
						onMouseDown={async (e) => {
							e.preventDefault();
							await claimProject();
						}}
					>
						🐰 This project is unclaimed!
						<br />
						Claim {name()}
					</button>
				</div>
			</div>
		</div>
	);
};

export default ClaimBanner;
