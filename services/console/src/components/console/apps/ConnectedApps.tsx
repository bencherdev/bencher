import * as Sentry from "@sentry/astro";
import type { Params } from "astro";
import {
	For,
	Match,
	Show,
	Switch,
	createResource,
	createSignal,
	onMount,
} from "solid-js";
import type { JsonOAuthGrant } from "../../../types/bencher";
import { authUser, isSameUser } from "../../../util/auth";
import { X_TOTAL_COUNT, httpDelete, httpGet } from "../../../util/http";
import { NotifyKind, pageNotify } from "../../../util/notify";
import { scopeText } from "../../auth/authorize/scope";
import {
	AppsRefusal,
	appsRefusal,
	appsRefusalText,
	lastRefreshed,
} from "./apps";

// The most the API lists at once.
const PER_PAGE = 255;

type Listed = { grants: JsonOAuthGrant[] } | { refusal: AppsRefusal };

export interface Props {
	apiUrl: string;
	params: Params;
}

const ConnectedApps = (props: Props) => {
	const grantsPath = () => `/v0/users/${props.params.user}/oauth/grants`;
	const [totalCount, setTotalCount] = createSignal(0);
	const [disconnected, setDisconnected] = createSignal<string>();
	let heading: undefined | HTMLHeadingElement;

	const list = (): Promise<Listed> => {
		const token = authUser()?.token;
		return httpGet(
			props.apiUrl,
			`${grantsPath()}?per_page=${PER_PAGE}&direction=desc`,
			token,
		)
			.then((resp) => {
				setTotalCount(Number(resp.headers?.[X_TOTAL_COUNT] ?? 0));
				return { grants: resp.data };
			})
			.catch((error) => {
				const refusal = appsRefusal(error?.response?.status, token);
				if (refusal === AppsRefusal.Failed) {
					console.error(error);
					Sentry.captureException(error);
				}
				return { refusal };
			});
	};
	const [listed, { refetch }] = createResource(list);

	const grants = () => {
		const current = listed();
		return current && "grants" in current ? current.grants : undefined;
	};
	const refusal = () => {
		const current = listed();
		return current && "refusal" in current ? current.refusal : undefined;
	};

	const revoke = (grant: JsonOAuthGrant) =>
		httpDelete(props.apiUrl, `${grantsPath()}/${grant.uuid}`, authUser()?.token)
			.then(() => true)
			.catch((error) => {
				// Already revoked elsewhere: it is gone either way.
				if (error?.response?.status === 404) {
					return true;
				}
				console.error(error);
				Sentry.captureException(error);
				pageNotify(
					NotifyKind.ERROR,
					`Lettuce romaine calm! Failed to revoke ${grant.client.host}. Try again.`,
				);
				return false;
			})
			.then(async (revoked) => {
				// The row stays in its revoking state until the list is the server's again.
				if (revoked) {
					await refetch();
					setDisconnected(`${grant.client.host} was disconnected.`);
					heading?.focus();
				}
			});

	return (
		<>
			<nav class="level">
				<div class="level-left">
					<div class="level-item">
						<h3 class="title is-3" tabIndex={-1} ref={heading}>
							<span class="icon-text">
								<span class="icon">
									<i class="fas fa-plug" />
								</span>
								<span>&nbsp;Connected Apps</span>
							</span>
						</h3>
					</div>
				</div>
			</nav>
			<p class="has-text-centered" role="status">
				{disconnected()}
			</p>
			<Switch fallback={<div class="box">&nbsp;</div>}>
				<Match when={refusal()}>
					{(refusal) => (
						<div class="content has-text-centered">
							<p>{appsRefusalText(refusal())}</p>
						</div>
					)}
				</Match>
				<Match when={grants()?.length === 0}>
					<div class="content has-text-centered">
						<h4>🐰 No connected apps</h4>
						<p>
							Apps you approve to use your Bencher account, such as AI
							assistants, show up here.
						</p>
					</div>
				</Match>
				<Match when={grants()}>
					{(grants) => (
						<>
							<For each={grants()}>
								{(grant) => (
									<ConnectedApp
										grant={grant}
										user={props.params.user}
										canRevoke={isSameUser(props.apiUrl, props.params)}
										revoke={revoke}
									/>
								)}
							</For>
							<Show when={totalCount() > grants().length}>
								<p class="has-text-centered">
									Showing the newest {grants().length} of {totalCount()}{" "}
									connected apps.
								</p>
							</Show>
						</>
					)}
				</Match>
			</Switch>
		</>
	);
};

const ConnectedApp = (props: {
	grant: JsonOAuthGrant;
	user: undefined | string;
	canRevoke: boolean;
	revoke: (grant: JsonOAuthGrant) => Promise<void>;
}) => {
	const [confirming, setConfirming] = createSignal(false);
	const [revoking, setRevoking] = createSignal(false);
	// Revoke takes focus back only when the confirmation it opened closes.
	const [returning, setReturning] = createSignal(false);
	const back = () => {
		setReturning(true);
		setConfirming(false);
	};
	const refreshed = () => lastRefreshed(props.grant);

	return (
		<article class="box" aria-label={props.grant.client.host}>
			<h4 class="title is-4">{props.grant.client.host}</h4>
			<p class="subtitle is-6">It calls itself “{props.grant.client.name}”.</p>
			<div class="content">
				<p>It can:</p>
				<ul>
					<For each={props.grant.scopes}>
						{(scope) => <li>{scopeText(scope)}</li>}
					</For>
				</ul>
				<p>
					Connected:{" "}
					<time datetime={props.grant.created}>
						{dateTime(props.grant.created)}
					</time>
				</p>
				<p>
					Last refreshed:{" "}
					<Show when={refreshed()} fallback="Never">
						{(refreshed) => (
							<time datetime={refreshed()}>{dateTime(refreshed())}</time>
						)}
					</Show>
				</p>
				<p>
					Approved:{" "}
					<Show when={props.grant.user_key} fallback="while logged in">
						{(key) => (
							<>
								with the API key{" "}
								<a href={`/console/users/${props.user}/keys/${key().uuid}`}>
									{key().name}
								</a>
								. Revoking or expiring that key disconnects the app.
							</>
						)}
					</Show>
				</p>
			</div>
			<Show when={props.canRevoke}>
				<Show
					when={confirming()}
					fallback={
						<div class="buttons is-right">
							<button
								class="button is-small"
								type="button"
								aria-label={`Revoke ${props.grant.client.host}`}
								ref={(el) => onMount(() => returning() && el.focus())}
								onClick={() => setConfirming(true)}
							>
								<span class="icon">
									<i class="fas fa-ban" />
								</span>
								<span>Revoke</span>
							</button>
						</div>
					}
				>
					<div class="content has-text-centered">
						<p>
							Are you sure? {props.grant.client.host} loses access to this
							account at once, and revocation is permanent.
						</p>
					</div>
					<div class="columns">
						<div class="column">
							<button
								class="button is-fullwidth"
								type="button"
								disabled={revoking()}
								onClick={async () => {
									setRevoking(true);
									await props.revoke(props.grant);
									setRevoking(false);
									back();
								}}
							>
								I am 💯 sure
							</button>
						</div>
						<div class="column">
							<button
								class="button is-primary is-fullwidth"
								type="button"
								ref={(el) => onMount(() => el.focus())}
								onClick={back}
							>
								Cancel
							</button>
						</div>
					</div>
				</Show>
			</Show>
		</article>
	);
};

const dateTime = (date: string) => new Date(date).toLocaleString();

export default ConnectedApps;
