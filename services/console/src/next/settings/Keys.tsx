import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Icon from "@bencherdev/ui/Icon";
import Segmented from "@bencherdev/ui/Segmented";
import Skeleton from "@bencherdev/ui/Skeleton";
import Table from "@bencherdev/ui/Table";
import TextInput from "@bencherdev/ui/TextInput";
import { useQueryClient } from "@tanstack/solid-query";
import { For, Match, Show, Switch, createMemo, createSignal } from "solid-js";
import type {
	JsonProjectKey,
	JsonProjectKeyCreated,
} from "../../types/bencher";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import { addKey, keysQuery, revokeKey } from "./data";
import { TOO_LONG, failureOf, tooLong } from "./general";
import Head from "./Head";
import {
	DEFAULT_EXPIRY,
	EXPIRIES,
	expiresAt,
	formatDay,
	keyEnd,
	newestFirst,
} from "./keys";

type Status = "active" | "revoked";

type Open =
	| { dialog: "new" }
	| { dialog: "reveal"; created: JsonProjectKeyCreated }
	| { dialog: "revoke"; key: JsonProjectKey };

/** The project keys runs report with: listed, made, shown once, and revoked. */
const Keys = () => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	const bootstrap = useQueryResult(() => consoleProjectQuery(api, slug()));
	// Only a Maintainer may list keys.
	const manage = () => bootstrap().data?.permissions.manage;
	const active = useQueryResult(() => ({
		...keysQuery(api, slug(), false),
		enabled: manage() !== false,
	}));
	const revoked = useQueryResult(() => ({
		...keysQuery(api, slug(), true),
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

	const revoke = async (key: JsonProjectKey) => {
		setFailure("");
		setNote(`Revoked ${key.name}. It cannot be used again.`);
		// A read of the active list in flight would land over the change.
		await client.cancelQueries({
			queryKey: ["console", "keys", slug(), "active"],
		});
		const unread = revoked().data === undefined;
		const undo = revokeKey(client, slug(), key.uuid, new Date().toISOString());
		// The row whose Revoke opened the dialog, and held focus after it, is gone.
		statusGroup?.querySelector<HTMLInputElement>("input:checked")?.focus();
		try {
			await api.send(
				"DELETE",
				`/v0/projects/${encodeURIComponent(slug())}/keys/${key.uuid}`,
			);
			if (unread) {
				// A first read still in flight is never restarted, only cancelled.
				const revokedKeys = {
					queryKey: ["console", "keys", slug(), "revoked"],
				};
				await client.cancelQueries(revokedKeys);
				client.invalidateQueries(revokedKeys);
			}
		} catch (error) {
			undo();
			setNote("");
			setFailure(
				failureOf(
					error,
					`Bencher did not revoke ${key.name}`,
					`The Bencher API did not answer, so ${key.name} still works.`,
				),
			);
		}
	};

	return (
		<>
			<Head
				slug={slug()}
				title="Keys"
				readOnly={manage() === false}
				actions={
					manage() ? (
						<Button
							variant="primary"
							aria-haspopup="dialog"
							onClick={() => setOpen({ dialog: "new" })}
						>
							New key
						</Button>
					) : undefined
				}
			>
				<p class="set-lede">
					A project key lets <span class="mono">bencher run</span> report to{" "}
					{bootstrap().data?.project.name ?? slug()}. Runs read it from{" "}
					<span class="mono">BENCHER_API_KEY</span>. Bencher shows a key once,
					when it is created.
				</p>
			</Head>
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
											Active <span class="count">{active().data?.total}</span>
										</>
									),
								},
								{
									value: "revoked",
									label: (
										<>
											Revoked <span class="count">{revoked().data?.total}</span>
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
							<p class="ferror" role="alert">
								{failure()}
							</p>
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
									<span class="grow">
										The keys did not load: the Bencher API did not answer.
									</span>
									<Button size="sm" onClick={() => listed().refetch()}>
										Retry
									</Button>
								</Banner>
							</Show>
						}
					>
						<KeyTable
							status={status()}
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
						onDismiss={close}
						onCreated={(created) => {
							addKey(client, slug(), created);
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
							project={bootstrap().data?.project.name ?? slug()}
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

export default Keys;

const KeyTable = (props: {
	status: Status;
	keys: JsonProjectKey[];
	made: ReadonlySet<string>;
	onRevoke: (key: JsonProjectKey) => void;
}) => {
	const now = Date.now();
	const isActive = () => props.status === "active";
	return (
		<Table
			fold
			class="keytbl"
			aria-label={isActive() ? "Active keys" : "Revoked keys"}
		>
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
								<p class="keys-empty">
									{isActive()
										? "No active keys. New key makes one."
										: "No revoked keys."}
								</p>
							</td>
						</tr>
					}
				>
					{(key) => (
						<tr>
							<td data-fold="l1">
								<b class="mono">{key.name}</b>
								<Show when={props.made.has(key.uuid)}>
									<span class="newtag">New</span>
								</Show>
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

const KeyEnd = (props: { keyed: JsonProjectKey; now: number }) => {
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
	onDismiss: () => void;
	onCreated: (created: JsonProjectKeyCreated) => void;
}) => {
	const { api, slug } = useProject();
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
			const { data } = await api.send<JsonProjectKeyCreated>(
				"POST",
				`/v0/projects/${encodeURIComponent(slug())}/keys`,
				{ name: name().trim(), ...(ttl === undefined ? {} : { ttl }) },
			);
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
					<Button variant="ghost" aria-label="Close" onClick={props.onDismiss}>
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
							aria-describedby="nk-name-h nk-name-e"
							aria-invalid={long() ? true : undefined}
						/>
						<span class="fhelp" id="nk-name-h">
							Up to 64 characters, so you can tell your keys apart.
						</span>
						<Show when={long()}>
							<p class="ferror" id="nk-name-e">
								{TOO_LONG}
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
								{failureOf(
									failure().error,
									"Bencher did not make the key",
									"The Bencher API did not answer, so no key was made.",
								)}
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
const Reveal = (props: {
	created: JsonProjectKeyCreated;
	onDone: () => void;
}) => {
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
					This is the only time Bencher shows{" "}
					<b class="mono">{props.created.name}</b>. Copy it now and store it as
					a secret; Bencher keeps only a hash of it.
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
					<legend class="flab">In a shell or a CI step</legend>
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
					{end().text === "Never"
						? "It never expires."
						: `Expires ${end().text}.`}{" "}
					You can revoke it from this page at any time.
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

const Revoke = (props: {
	target: JsonProjectKey;
	project: string;
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
				<Button variant="ghost" aria-label="Close" onClick={props.onDismiss}>
					<Icon name="close" />
				</Button>
			</div>
			<div class="ui-dialog-body">
				<p id="rk-desc" class="text2">
					Every run that uses this key stops reporting to {props.project} at
					once. <b>Revoking cannot be undone:</b> a revoked key never works
					again, and those runs need a new key.
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
					Revoke key
				</Button>
			</div>
		</Dialog>
	);
};
