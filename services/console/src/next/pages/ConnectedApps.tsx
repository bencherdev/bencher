import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import { useNavigate, useParams } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { For, Show, createSignal } from "solid-js";
import { scopeText } from "../../components/auth/authorize/scope";
import type { JsonOAuthGrant } from "../../types/bencher";
import {
	type GrantPage,
	day,
	dropGrant,
	grantPath,
	grantsQuery,
	lastRefreshed,
	refusalOf,
} from "../account/apps";
import { useAccount } from "../account/context";
import { accountPath } from "../account/paths";
import { ApiError } from "../api";
import { useQueryResult } from "../query";
import { Frame, failureOf } from "./Profile";
import { formatCount } from "../plot/count";

/** The apps connected to the reader's account, each revoked after a confirmation. */
const ConnectedApps = () => {
	const { api, reader } = useAccount();
	const params = useParams();
	const navigate = useNavigate();
	const user = () => params.user ?? reader().user.slug;
	// The shortcut names no user; the reader's own page has the same list.
	if (!params.user) {
		navigate(`/users/${user()}/apps`, { replace: true });
	}
	const client = useQueryClient();
	const list = useQueryResult(() => grantsQuery(api, user()));
	const refusal = () => refusalOf(list().error, reader().token);
	const [target, setTarget] = createSignal<JsonOAuthGrant>();
	const [note, setNote] = createSignal("");
	const [failure, setFailure] = createSignal("");
	let heading: HTMLHeadingElement | undefined;

	const revoke = async (grant: JsonOAuthGrant) => {
		const host = grant.client.host;
		setFailure("");
		setNote(`Revoked ${host}`);
		// A read in flight would land over the change.
		await client.cancelQueries({ queryKey: grantsQuery(api, user()).queryKey });
		const undo = dropGrant(client, api, user(), grant.uuid);
		// The app whose Revoke held focus is gone.
		heading?.focus();
		try {
			await api.send("DELETE", grantPath(user(), grant.uuid));
		} catch (error) {
			// Revoked elsewhere already: it is gone either way.
			if (!(error instanceof ApiError && error.kind === "not_found")) {
				undo();
				setNote("");
				setFailure(failureOf(error, `Bencher did not revoke ${host}`));
				return;
			}
		}
		// A list the API cut short has more apps to show in the room made.
		const { queryKey } = grantsQuery(api, user());
		const page = client.getQueryData<GrantPage>(queryKey);
		if (page && page.total > page.grants.length) {
			client.invalidateQueries({ queryKey });
		}
	};

	return (
		<Frame user={user()} section="apps">
			<div class="sethead">
				<div class="ph-title">
					<div class="crumb-line">
						Account<span aria-hidden="true">/</span>
					</div>
					<Heading
						level={1}
						size="xl"
						tabindex={-1}
						ref={(element: HTMLHeadingElement) => {
							heading = element;
						}}
					>
						Connected Apps
					</Heading>
				</div>
				<div class="ph-actions">
					<span class="set-status" role="status">
						<Show when={note()}>
							<Icon name="check" />
							{note()}
						</Show>
					</span>
				</div>
			</div>
			<Show when={failure()}>
				<p class="ferror" role="alert">
					{failure()}
				</p>
			</Show>
			<Show
				when={list().data}
				fallback={
					<Show
						when={refusal()}
						fallback={
							<Show when={list().error} fallback={<Skeleton size="card" />}>
								<Banner status="error" role="alert" class="load-error">
									<span class="grow">The connected apps did not load</span>
									<Button size="sm" onClick={() => list().refetch()}>
										Retry
									</Button>
								</Banner>
							</Show>
						}
					>
						{(why) => (
							<div class="setcard">
								<p class="keys-empty">{why()}</p>
							</div>
						)}
					</Show>
				}
			>
				{(page) => (
					<div class="setbody tight">
						<For
							each={page().grants}
							fallback={
								// Emptied by revokes, a cut short list waits for the apps past it.
								<Show
									when={page().total === 0}
									fallback={<Skeleton size="card" />}
								>
									<div class="setcard">
										<p class="keys-empty">No connected apps</p>
									</div>
								</Show>
							}
						>
							{(grant) => (
								<App
									grant={grant}
									keys={accountPath(user(), "keys")}
									onRevoke={() => setTarget(grant)}
								/>
							)}
						</For>
						<Show
							when={
								page().grants.length > 0 && page().total > page().grants.length
							}
						>
							<p class="muted sm">
								Newest {page().grants.length} of {formatCount(page().total)}
							</p>
						</Show>
					</div>
				)}
			</Show>
			<Show when={target()} keyed>
				{(grant) => (
					<Dialog
						role="alertdialog"
						aria-labelledby="ra-title"
						aria-describedby="ra-desc"
						onDismiss={() => setTarget(undefined)}
					>
						<div class="ui-dialog-head">
							<h2 id="ra-title">Revoke {grant.client.host}?</h2>
							<Button
								variant="ghost"
								iconOnly
								aria-label="Close"
								onClick={() => setTarget(undefined)}
							>
								<Icon name="close" />
							</Button>
						</div>
						<div class="ui-dialog-body">
							<p id="ra-desc" class="text2">
								{grant.client.host} loses access to this account at once.
							</p>
						</div>
						<div class="ui-dialog-foot">
							<Button onClick={() => setTarget(undefined)}>Cancel</Button>
							<Button
								variant="destructive"
								onClick={() => {
									setTarget(undefined);
									revoke(grant);
								}}
							>
								Revoke app
							</Button>
						</div>
					</Dialog>
				)}
			</Show>
		</Frame>
	);
};

export default ConnectedApps;

/** One app: who it is, what it can do, and how it was approved. */
const App = (props: {
	grant: JsonOAuthGrant;
	/** The reader's Keys page, where the key an app was approved with is listed. */
	keys: string;
	onRevoke: () => void;
}) => {
	const id = () => `app-${props.grant.uuid}`;
	const refreshed = () => lastRefreshed(props.grant);
	return (
		<section class="setcard" aria-labelledby={id()}>
			<div class="setcard-head">
				<Heading level={2} id={id()} class="mono">
					{props.grant.client.host}
				</Heading>
				<span class="spacer" />
				<button
					type="button"
					class="lnk danger"
					aria-haspopup="dialog"
					aria-label={`Revoke ${props.grant.client.host}`}
					onClick={() => props.onRevoke()}
				>
					Revoke
				</button>
			</div>
			<div class="setcard-body">
				<dl>
					<div class="kv">
						<dt>calls itself</dt>
						<dd>{props.grant.client.name}</dd>
					</div>
					<div class="kv">
						<dt>can</dt>
						<dd>
							<ul
								style={{
									"padding-inline-start": "18px",
									"font-weight": "var(--font-weight-normal)",
								}}
							>
								<For each={props.grant.scopes}>
									{(scope) => <li>{scopeText(scope)}</li>}
								</For>
							</ul>
						</dd>
					</div>
					<div class="kv">
						<dt>connected</dt>
						<dd>
							<Day at={props.grant.created} />
						</dd>
					</div>
					<div class="kv">
						<dt>refreshed</dt>
						<dd>
							<Show when={refreshed()} fallback="Never">
								{(at) => <Day at={at()} />}
							</Show>
						</dd>
					</div>
					<div class="kv">
						<dt>approved</dt>
						<dd>
							<Show when={props.grant.user_key} fallback="while signed in">
								{(key) => (
									<>
										with key <a href={props.keys}>{key().name}</a>
									</>
								)}
							</Show>
						</dd>
					</div>
				</dl>
			</div>
		</section>
	);
};

const Day = (props: { at: string }) => (
	<time datetime={props.at}>{day(props.at)}</time>
);
