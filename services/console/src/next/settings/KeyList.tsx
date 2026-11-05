import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Icon from "@bencherdev/ui/Icon";
import Segmented from "@bencherdev/ui/Segmented";
import Skeleton from "@bencherdev/ui/Skeleton";
import Table from "@bencherdev/ui/Table";
import TextInput from "@bencherdev/ui/TextInput";
import Truncate from "@bencherdev/ui/Truncate";
import { useQueryClient } from "@tanstack/solid-query";
import {
	For,
	type JSX,
	Match,
	Show,
	Switch,
	createMemo,
	createSignal,
} from "solid-js";
import type { Api } from "../api";
import { useQueryResult } from "../query";
import { type KeyHome, addKey, keysKey, keysQuery, revokeKey } from "./data";
import { failureOf, overBytes, tooLong } from "./general";
import {
	DEFAULT_EXPIRY,
	EXPIRIES,
	type KeyRow,
	expiresAt,
	formatDay,
	keyEnd,
	newestFirst,
} from "./keys";
import { formatCount } from "../plot/count";

type Status = "active" | "revoked";

/** A key just made, with the secret it shows once. */
type Created = KeyRow & { key: string };

type Open =
	| { dialog: "new" }
	| { dialog: "reveal"; created: Created }
	| { dialog: "revoke"; key: KeyRow };

/** A list of keys: listed, made, shown once, and revoked. */
const KeyList = (props: {
	api: Api;
	home: KeyHome;
	/** Whether the reader may list and make keys; unknown until the shell answers. */
	manage: boolean | undefined;
	head: (actions: JSX.Element | undefined) => JSX.Element;
	/** What a revoke ends, said before it does. */
	consequence: string;
}) => {
	const client = useQueryClient();
	const manage = () => props.manage;
	const active = useQueryResult(() => ({
		...keysQuery(props.api, props.home, false),
		enabled: manage() !== false,
	}));
	const revoked = useQueryResult(() => ({
		...keysQuery(props.api, props.home, true),
		enabled: manage() !== false,
	}));

	const [status, setStatus] = createSignal<Status>("active");
	const [open, setOpen] = createSignal<Open>();
	const [made, setMade] = createSignal<ReadonlySet<string>>(new Set());
	const [note, setNote] = createSignal("");
	const [failure, setFailure] = createSignal("");
	const close = () => setOpen(undefined);
	let statusGroup: HTMLDivElement | undefined;

	const listed = () => (status() === "active" ? active() : revoked());
	const shown = createMemo(() => newestFirst(listed().data?.keys ?? []));

	const revoke = async (key: KeyRow) => {
		const home = props.home;
		setFailure("");
		setNote("Revoked");
		// A read of the active list in flight would land over the change.
		await client.cancelQueries({ queryKey: keysKey(home, false) });
		const unread = revoked().data === undefined;
		const undo = revokeKey(client, home, key.uuid, new Date().toISOString());
		// The row whose Revoke opened the dialog, and held focus after it, is gone.
		statusGroup?.querySelector<HTMLInputElement>("input:checked")?.focus();
		try {
			await props.api.send("DELETE", `${home.path}/${key.uuid}`);
			if (unread) {
				// A first read still in flight is never restarted, only cancelled.
				const revokedKeys = { queryKey: keysKey(home, true) };
				await client.cancelQueries(revokedKeys);
				client.invalidateQueries(revokedKeys);
			}
		} catch (error) {
			undo();
			setNote("");
			setFailure(failureOf(error, "Not revoked"));
		}
	};

	return (
		<>
			{props.head(
				manage() ? (
					<Button
						variant="primary"
						aria-haspopup="dialog"
						onClick={() => setOpen({ dialog: "new" })}
					>
						New key
					</Button>
				) : undefined,
			)}
			<Show when={manage() !== false}>
				<div class="setbody tight">
					<div class="toolbar">
						<Segmented
							ref={(element: HTMLDivElement) => {
								statusGroup = element;
							}}
							name="key-status"
							aria-label="Status"
							value={status()}
							onChange={setStatus}
							options={[
								{
									value: "active",
									label: (
										<>
											Active{" "}
											<span class="count">
												{formatCount(active().data?.total)}
											</span>
										</>
									),
								},
								{
									value: "revoked",
									label: (
										<>
											Revoked{" "}
											<span class="count">
												{formatCount(revoked().data?.total)}
											</span>
										</>
									),
								},
							]}
						/>
						<span class="set-status" role="status">
							<Show when={note()}>
								<Icon name="check" />
								{note()}
							</Show>
						</span>
						<Show when={failure()}>
							{(message) => (
								<p class="ferror" role="alert">
									<Truncate text={message()} reveal />
								</p>
							)}
						</Show>
					</div>
					<Show
						when={listed().data}
						fallback={
							<Show
								when={manage() && listed().error}
								fallback={<Skeleton size="card" />}
							>
								<Banner status="error" role="alert" class="load-error">
									<span class="grow">The keys did not load</span>
									<Button size="sm" onClick={() => listed().refetch()}>
										Retry
									</Button>
								</Banner>
							</Show>
						}
					>
						<KeyTable
							status={status()}
							label={status() === "active" ? "Active keys" : "Revoked keys"}
							empty="No keys"
							keys={shown()}
							made={made()}
							onRevoke={(key) => setOpen({ dialog: "revoke", key })}
						/>
					</Show>
				</div>
			</Show>
			<Switch>
				<Match when={open()?.dialog === "new"}>
					<NewKey
						api={props.api}
						path={props.home.path}
						onDismiss={close}
						onCreated={(created) => {
							addKey(client, props.home, created);
							setMade((keys) => new Set(keys).add(created.uuid));
							setStatus("active");
							setOpen({ dialog: "reveal", created });
						}}
					/>
				</Match>
				<Match
					when={(() => {
						const current = open();
						return current?.dialog === "reveal" ? current.created : undefined;
					})()}
					keyed
				>
					{(created) => <Reveal created={created} onDone={close} />}
				</Match>
				<Match
					when={(() => {
						const current = open();
						return current?.dialog === "revoke" ? current.key : undefined;
					})()}
					keyed
				>
					{(target) => (
						<Revoke
							target={target}
							consequence={props.consequence}
							action="Revoke key"
							onDismiss={close}
							onRevoke={() => {
								close();
								revoke(target);
							}}
						/>
					)}
				</Match>
			</Switch>
		</>
	);
};

export default KeyList;

/** Keys as rows: name, created, and when each ends or was revoked. */
export const KeyTable = (props: {
	status: Status;
	label: string;
	empty: string;
	keys: KeyRow[];
	made: ReadonlySet<string>;
	onRevoke: (key: KeyRow) => void;
}) => {
	const now = Date.now();
	const isActive = () => props.status === "active";
	return (
		<Table fold={407} class="keytbl" aria-label={props.label}>
			<thead>
				<tr>
					<th scope="col">Name</th>
					<th scope="col">Created</th>
					<th scope="col">{isActive() ? "Expires" : "Revoked"}</th>
					<th scope="col">
						<span class="sr-only">Actions</span>
					</th>
				</tr>
			</thead>
			<tbody>
				<For
					each={props.keys}
					fallback={
						<tr>
							<td colSpan={4} data-fold="l1">
								<p class="keys-empty">{props.empty}</p>
							</td>
						</tr>
					}
				>
					{(key) => (
						<tr>
							<td data-fold="l1" class="ui-table-name">
								<span class="ui-parts">
									<b class="mono">
										<Truncate text={key.name} reveal />
									</b>
									<Show when={props.made.has(key.uuid)}>
										<span class="newtag" data-fixed>
											New
										</span>
									</Show>
								</span>
							</td>
							<td data-fold="l2" class="muted">
								<span class="ui-fold-label">Created </span>
								{formatDay(Date.parse(key.creation))}
							</td>
							<td data-fold="n1">
								<Show
									when={key.revoked}
									fallback={<KeyEnd keyed={key} now={now} />}
								>
									{(at) => (
										<span class="muted">
											<span class="ui-fold-label">Revoked </span>
											{formatDay(Date.parse(at()))}
										</span>
									)}
								</Show>
							</td>
							<td data-fold="act" class="act">
								<Show when={isActive()}>
									<button
										type="button"
										class="lnk danger"
										aria-haspopup="dialog"
										aria-label={`Revoke ${key.name}`}
										onClick={() => props.onRevoke(key)}
									>
										Revoke
									</button>
								</Show>
							</td>
						</tr>
					)}
				</For>
			</tbody>
		</Table>
	);
};

const KeyEnd = (props: { keyed: KeyRow; now: number }) => {
	const end = () => keyEnd(props.keyed, props.now);
	return (
		<span class={end().expired ? "faint" : undefined}>
			<Show when={!end().expired && end().text !== "Never"}>
				<span class="ui-fold-label">Expires </span>
			</Show>
			{end().text}
			<Show when={end().text === "Never"}>
				<span class="ui-fold-label"> expires</span>
			</Show>
		</span>
	);
};

const NewKey = (props: {
	api: Api;
	/** Where a key is made. */
	path: string;
	onDismiss: () => void;
	onCreated: (created: Created) => void;
}) => {
	const [name, setName] = createSignal("");
	const [expiry, setExpiry] = createSignal(DEFAULT_EXPIRY);
	// The API counts bytes, so a name of accented letters runs out sooner.
	const long = () => tooLong(name().trim());
	const now = Date.now();
	const [creating, setCreating] = createSignal(false);
	const [failed, setFailed] = createSignal<{ error: unknown }>();
	const create = async () => {
		const ttl = EXPIRIES.find(({ id }) => id === expiry())?.ttl;
		setCreating(true);
		setFailed(undefined);
		try {
			const { data } = await props.api.send<Created>("POST", props.path, {
				name: name().trim(),
				...(ttl === undefined ? {} : { ttl }),
			});
			props.onCreated(data);
		} catch (error) {
			setFailed({ error });
			setCreating(false);
		}
	};
	return (
		<Dialog aria-labelledby="nk-title" onDismiss={props.onDismiss}>
			<form
				class="ui-dialog-form"
				onSubmit={(event) => {
					event.preventDefault();
					if (name().trim() && !long() && !creating()) {
						create();
					}
				}}
			>
				<div class="ui-dialog-head">
					<h2 id="nk-title">New key</h2>
					<Button
						variant="ghost"
						iconOnly
						aria-label="Close"
						onClick={props.onDismiss}
					>
						<Icon name="close" />
					</Button>
				</div>
				<div class="ui-dialog-body">
					<div class="field">
						<label class="flab" for="nk-name">
							Name
						</label>
						<TextInput
							id="nk-name"
							value={name()}
							onInput={(event) => setName(event.currentTarget.value)}
							placeholder="release-benchmarks"
							autocomplete="off"
							autofocus
							aria-describedby="nk-name-e"
							aria-invalid={long() ? true : undefined}
						/>
						<Show when={long()}>
							<p class="ferror" id="nk-name-e">
								{overBytes(name().trim())}
							</p>
						</Show>
					</div>
					<fieldset class="fset field">
						<legend class="flab">Expires</legend>
						<div class="radios">
							<For each={EXPIRIES}>
								{(option) => {
									const at = expiresAt(now, option);
									return (
										<label class="radio">
											<input
												type="radio"
												name="nk-ttl"
												checked={expiry() === option.id}
												onChange={() => setExpiry(option.id)}
											/>
											<span>{option.label}</span>
											<Show when={at}>
												{(day) => (
													<span class="muted sm">{formatDay(day())}</span>
												)}
											</Show>
										</label>
									);
								}}
							</For>
						</div>
					</fieldset>
					<Show when={failed()}>
						{(failure) => (
							<p class="ferror" role="alert">
								<Truncate
									text={failureOf(
										failure().error,
										"Bencher did not make the key",
									)}
									reveal
								/>
							</p>
						)}
					</Show>
				</div>
				<div class="ui-dialog-foot">
					<Button onClick={props.onDismiss}>Cancel</Button>
					<Button
						type="submit"
						variant="primary"
						disabled={!name().trim() || long() || creating()}
					>
						Create key
					</Button>
				</div>
			</form>
		</Dialog>
	);
};

/**
 * The one time the secret is on screen; it lives only here, never in the
 * cache, and only Done closes it, so a stray Escape cannot lose it.
 */
const Reveal = (props: { created: Created; onDone: () => void }) => {
	const [copied, setCopied] = createSignal<"key" | "line">();
	const line = () => `export BENCHER_API_KEY=${props.created.key}`;
	const copy = (what: "key" | "line", text: string) =>
		navigator.clipboard.writeText(text).then(
			() => setCopied(what),
			() => setCopied(undefined),
		);
	const end = () => keyEnd(props.created, Date.now());
	return (
		<Dialog aria-labelledby="rv-title" aria-describedby="rv-desc">
			<div class="ui-dialog-head">
				<h2 id="rv-title">Copy your new key</h2>
			</div>
			<div class="ui-dialog-body">
				<p id="rv-desc" class="text2">
					<b class="mono">{props.created.name}</b> · shown once
				</p>
				<div class="secret">
					<span>{props.created.key}</span>
					<Button
						size="sm"
						aria-label={copied() === "key" ? "Key copied" : "Copy the key"}
						onClick={() => copy("key", props.created.key)}
					>
						{copied() === "key" ? "Copied" : "Copy"}
					</Button>
				</div>
				<fieldset class="fset field">
					<legend class="flab">Export line</legend>
					<div class="code">
						<span>{line()}</span>
						<Button
							size="sm"
							aria-label={
								copied() === "line" ? "Line copied" : "Copy the export line"
							}
							onClick={() => copy("line", line())}
						>
							{copied() === "line" ? "Copied" : "Copy"}
						</Button>
					</div>
				</fieldset>
				<p class="fhelp">
					{end().text === "Never" ? "Never expires" : `Expires ${end().text}`}
				</p>
			</div>
			<div class="ui-dialog-foot">
				<Button variant="primary" onClick={props.onDone}>
					Done
				</Button>
			</div>
		</Dialog>
	);
};

/** Asks before a revoke, saying what it ends. */
export const Revoke = (props: {
	target: KeyRow;
	consequence: string;
	/** The confirming button's label. */
	action: string;
	onDismiss: () => void;
	onRevoke: () => void;
}) => {
	const end = () => keyEnd(props.target, Date.now());
	return (
		<Dialog
			role="alertdialog"
			aria-labelledby="rk-title"
			aria-describedby="rk-desc"
			onDismiss={props.onDismiss}
		>
			<div class="ui-dialog-head">
				<h2 id="rk-title">Revoke {props.target.name}?</h2>
				<Button
					variant="ghost"
					iconOnly
					aria-label="Close"
					onClick={props.onDismiss}
				>
					<Icon name="close" />
				</Button>
			</div>
			<div class="ui-dialog-body">
				<p id="rk-desc" class="text2">
					{props.consequence}
				</p>
				<dl>
					<div class="kv">
						<dt>created</dt>
						<dd>{formatDay(Date.parse(props.target.creation))}</dd>
					</div>
					<div class="kv">
						<dt>expires</dt>
						<dd>{end().text}</dd>
					</div>
				</dl>
			</div>
			<div class="ui-dialog-foot">
				<Button onClick={props.onDismiss}>Cancel</Button>
				<Button variant="destructive" onClick={props.onRevoke}>
					{props.action}
				</Button>
			</div>
		</Dialog>
	);
};
