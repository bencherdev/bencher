import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import Truncate from "@bencherdev/ui/Truncate";
import { useParams } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { Show, createMemo, createSignal } from "solid-js";
import { useAccount } from "../account/context";
import { profilePath } from "../paths";
import { useQueryResult } from "../query";
import { Frame, legacyTokensQuery } from "./Profile";
import {
	type KeyRow,
	KeyTable,
	Revoke,
	failureOf,
	keysKey,
	keysQuery,
	newestFirst,
	revokeKey,
} from "./Settings";
import { Head, legacyTokenList } from "./UserKeys";

/** The reader's unrevoked legacy tokens, which can only be revoked. */
const LegacyTokens = () => {
	const { api } = useAccount();
	const params = useParams();
	const user = () => params.user ?? "";
	const client = useQueryClient();
	const list = useQueryResult(() =>
		keysQuery(api, legacyTokenList(user()), false),
	);
	const shown = createMemo(() => newestFirst(list().data?.keys ?? []));
	const [target, setTarget] = createSignal<KeyRow>();
	const [note, setNote] = createSignal("");
	const [failure, setFailure] = createSignal("");
	let heading: HTMLHeadingElement | undefined;

	const revoke = async (token: KeyRow) => {
		const home = legacyTokenList(user());
		setFailure("");
		setNote("Revoked");
		await client.cancelQueries({ queryKey: keysKey(home, false) });
		const undo = revokeKey(client, home, token.uuid, new Date().toISOString());
		// The row whose Revoke held focus is gone.
		heading?.focus();
		try {
			await api.send("DELETE", `${home.path}/${token.uuid}`);
			client.setQueryData<number>(
				legacyTokensQuery(api, user()).queryKey,
				(count) => (count === undefined ? count : count - 1),
			);
		} catch (error) {
			undo();
			setNote("");
			setFailure(failureOf(error, "Not revoked"));
		}
	};

	return (
		<Frame user={user()} section="profile">
			<Head
				title="Legacy tokens"
				crumbs={[{ label: "Profile", href: profilePath(user()) }]}
				ref={(element) => {
					heading = element;
				}}
				actions={
					<span class="set-status" role="status">
						<Show when={note()}>
							<Icon name="check" />
							{note()}
						</Show>
					</span>
				}
			/>
			<Show when={failure()}>
				{(message) => (
					<p class="ferror" role="alert">
						<Truncate text={message()} reveal />
					</p>
				)}
			</Show>
			<Show
				when={list().data}
				fallback={
					<Show when={list().error} fallback={<Skeleton size="card" />}>
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">The legacy tokens did not load</span>
							<Button size="sm" onClick={() => list().refetch()}>
								Retry
							</Button>
						</Banner>
					</Show>
				}
			>
				<KeyTable
					status="active"
					label="Legacy API tokens"
					empty="No legacy tokens"
					keys={shown()}
					made={new Set()}
					onRevoke={setTarget}
				/>
			</Show>
			<Show when={target()} keyed>
				{(token) => (
					<Revoke
						target={token}
						consequence="Anything using it stops working, for good."
						action="Revoke token"
						onDismiss={() => setTarget(undefined)}
						onRevoke={() => {
							setTarget(undefined);
							revoke(token);
						}}
					/>
				)}
			</Show>
		</Frame>
	);
};

export default LegacyTokens;
