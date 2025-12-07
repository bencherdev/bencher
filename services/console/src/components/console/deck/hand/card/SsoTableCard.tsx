import { For, type Resource, Show } from "solid-js";
import type { JsonOrganization } from "../../../../../types/bencher";

export interface Props {
	isBencherCloud: boolean;
	value: Resource<JsonOrganization>;
}

const SsoTableCard = (props: Props) => {
	return (
		<div class="box" style="margin-top: 2rem">
			<h2 class="title is-4">Single Sign-On (SSO)</h2>
			<Show
				when={(props.value()?.sso?.length ?? 0) === 0}
				fallback={
					<>
						<For each={props.value()?.sso}>
							{(sso) => (
								<div class="box" style="margin-bottom: 1rem;">
									<div>
										<span class="icon-text">
											<span class="icon">
												<i class="fas fa-globe" />
											</span>
											<small style="word-break: break-word;">
												{sso?.domain}
											</small>
										</span>
									</div>
								</div>
							)}
						</For>
						<Show
							when={props.isBencherCloud}
							fallback={
								<p>
									The Bencher Self-Hosted administrator can make changes to your
									SSO configuration using{" "}
									<a href="https://bencher.dev/docs/api/organizations/sso/">
										the SSO REST API
									</a>
									.
								</p>
							}
						>
							<p>
								Contact us at{" "}
								<a href="mailto:enterprise@bencher.dev">Enterprise Support</a>{" "}
								to make changes to your SSO configuration.
							</p>
						</Show>
					</>
				}
			>
				<div class="box" style="margin-bottom: 1rem;">
					<h3 class="title is-4">🐰 No SSO domains configured</h3>
					<p>In order to add SSO, you must have a Bencher Enterprise plan.</p>
					<a
						type="button"
						class="button is-primary is-half"
						style="margin-top: 1rem; margin-bottom: 1rem;"
						href={`/console/organizations/${props.value()?.slug}/billing?plan=enterprise`}
					>
						Upgrade to Bencher Enterprise
					</a>
					<Show
						when={props.isBencherCloud}
						fallback={
							<p>
								Then use{" "}
								<a href="https://bencher.dev/docs/api/organizations/sso/">
									the SSO REST API
								</a>{" "}
								to add your SSO domains.
							</p>
						}
					>
						<p>
							Then contact us at{" "}
							<a href="mailto:enterprise@bencher.dev">Enterprise Support</a>.
						</p>
					</Show>
				</div>
			</Show>
		</div>
	);
};

export default SsoTableCard;
