import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import Table from "@bencherdev/ui/Table";
import TextInput from "@bencherdev/ui/TextInput";
import Truncate from "@bencherdev/ui/Truncate";
import { useParams } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { For, Show, createSignal } from "solid-js";
import type { JsonMember, JsonOrganization } from "../../types/bencher";
import { useAccount } from "../account/context";
import {
	type Roles,
	added,
	invite,
	memberCount,
	membersQuery,
	removeMember,
	rolesQuery,
	sendable,
} from "../organization/members";
import { consoleUserQuery } from "../queries";
import { useQueryResult } from "../query";
import {
	Frame,
	failureOf,
	organizationIn,
	organizationQuery,
} from "./OrgGeneral";

/** The organization's members, with Invite and Remove, and its single sign-on. */
const OrgMembers = () => {
	const { api, reader } = useAccount();
	const client = useQueryClient();
	const params = useParams();
	const key = () => params.organization ?? "";
	const organization = useQueryResult(() => organizationQuery(api, key()));
	const members = useQueryResult(() => membersQuery(api, key()));
	const roles = useQueryResult(() => rolesQuery(api, key()));
	// Only read: an organization the reader has seen names the rail at once.
	const bootstrap = useQueryResult(() => ({
		...consoleUserQuery(api, reader().user.uuid),
		enabled: false,
	}));
	const name = () =>
		organization().data?.name ?? organizationIn(bootstrap().data, key())?.name;
	// Single sign-on is only on the organization itself, so nothing draws before it.
	const ready = () => {
		const shown = organization().data;
		const list = members().data;
		const may = roles().data;
		return shown && list && may
			? { organization: shown, members: list.members, roles: may }
			: undefined;
	};
	const [note, setNote] = createSignal("");
	const [failure, setFailure] = createSignal("");
	const [target, setTarget] = createSignal<JsonMember>();
	let heading: HTMLHeadingElement | undefined;

	const remove = async (member: JsonMember) => {
		setNote("");
		setFailure("");
		try {
			await removeMember(api, client, key(), member.uuid);
			setNote("Removed");
			// The row whose Remove held focus is gone.
			heading?.focus();
		} catch (error) {
			setFailure(failureOf(error, "Not removed"));
		}
	};

	return (
		<Frame organization={key()} name={name()} section="members">
			<div class="sethead">
				<div class="ph-title">
					<div class="crumb-line">
						Settings<span aria-hidden="true">/</span>
					</div>
					<Heading
						level={1}
						size="xl"
						tabindex={-1}
						ref={(element: HTMLHeadingElement) => {
							heading = element;
						}}
					>
						Members
					</Heading>
					<p class="ph-sub">
						{members().data
							? memberCount(members().data?.total ?? 0)
							: "\u00a0"}
					</p>
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
				{(message) => (
					<p class="ferror" role="alert">
						<Truncate text={message()} reveal />
					</p>
				)}
			</Show>
			<Show
				when={ready()}
				fallback={
					<Show
						when={organization().error ?? members().error ?? roles().error}
						fallback={<Skeleton size="card" />}
					>
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">Members did not load</span>
							<Button
								size="sm"
								onClick={() => {
									organization().refetch();
									members().refetch();
									roles().refetch();
								}}
							>
								Retry
							</Button>
						</Banner>
					</Show>
				}
			>
				{(shown) => (
					<>
						<Show when={shown().roles.invite}>
							<Invite
								organization={key()}
								onSent={() => {
									setFailure("");
									setNote("Invitation sent");
								}}
							/>
						</Show>
						<MemberTable
							members={shown().members}
							roles={shown().roles}
							reader={reader().user.uuid}
							onRemove={setTarget}
						/>
						<SingleSignOn organization={shown().organization} />
					</>
				)}
			</Show>
			<Show when={target()} keyed>
				{(member) => (
					<Dialog
						role="alertdialog"
						aria-labelledby="rm-title"
						aria-describedby="rm-desc"
						onDismiss={() => setTarget(undefined)}
					>
						<div class="ui-dialog-head">
							<h2 id="rm-title">Remove {member.name}?</h2>
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
							<p id="rm-desc" class="text2">
								{member.name} ({member.email}) loses access to every project in{" "}
								{name() ?? key()}.
							</p>
						</div>
						<div class="ui-dialog-foot">
							<Button onClick={() => setTarget(undefined)}>Cancel</Button>
							<Button
								variant="destructive"
								onClick={() => {
									setTarget(undefined);
									remove(member);
								}}
							>
								Remove member
							</Button>
						</div>
					</Dialog>
				)}
			</Show>
		</Frame>
	);
};

export default OrgMembers;

const Invite = (props: { organization: string; onSent: () => void }) => {
	const { api } = useAccount();
	const [email, setEmail] = createSignal("");
	const [sending, setSending] = createSignal(false);
	const [failure, setFailure] = createSignal("");
	let input: HTMLInputElement | undefined;
	const send = async () => {
		const to = email().trim();
		setSending(true);
		setFailure("");
		try {
			await invite(api, props.organization, to);
			setEmail("");
			props.onSent();
			// Send invite disables itself once the field is empty.
			input?.focus();
		} catch (error) {
			setFailure(failureOf(error, "Bencher did not send the invitation"));
		} finally {
			setSending(false);
		}
	};
	return (
		<form
			class="setcard"
			aria-labelledby="sec-invite"
			onSubmit={(event) => {
				event.preventDefault();
				if (sendable(email())) {
					send();
				}
			}}
		>
			<div class="setcard-head">
				<h2 class="seclabel" id="sec-invite">
					Invite
				</h2>
			</div>
			<div class="setcard-body">
				<div class="toolbar inrow">
					<label class="sr-only" for="inv-email">
						Email
					</label>
					<TextInput
						id="inv-email"
						type="email"
						value={email()}
						onInput={(event) => {
							setEmail(event.currentTarget.value);
							setFailure("");
						}}
						placeholder="name@company.com"
						autocomplete="off"
						ref={(element: HTMLInputElement) => {
							input = element;
						}}
					/>
					<Button
						type="submit"
						variant="primary"
						disabled={!sendable(email()) || sending()}
					>
						Send invite
					</Button>
				</div>
				<Show when={failure()}>
					{(message) => (
						<p class="ferror" role="alert">
							<Truncate text={message()} reveal />
						</p>
					)}
				</Show>
			</div>
		</form>
	);
};

const MemberTable = (props: {
	members: JsonMember[];
	roles: Roles;
	reader: string;
	onRemove: (member: JsonMember) => void;
}) => (
	<Table fold={369} aria-label="Members">
		<thead>
			<tr>
				<th scope="col">Name</th>
				<th scope="col">Email</th>
				<th scope="col">
					<span class="sr-only">Actions</span>
				</th>
			</tr>
		</thead>
		<tbody>
			<For each={props.members}>
				{(member) => {
					const you = () => member.uuid === props.reader;
					return (
						<tr>
							<td data-fold="l1" class="ui-table-name">
								<span class="ui-parts">
									<b>
										<Truncate text={member.name} reveal />
									</b>
									<Show when={you()}>
										{" "}
										<span class="muted sm" data-fixed>
											you
										</span>
									</Show>
								</span>
							</td>
							<td data-fold="l2" class="muted ui-table-name">
								<span class="ui-parts">
									<Truncate text={member.email} reveal />
								</span>
							</td>
							<td data-fold="act" class="act">
								<Show when={props.roles.remove && !you()}>
									<button
										type="button"
										class="lnk danger"
										aria-haspopup="dialog"
										aria-label={`Remove ${member.name} (${member.email})`}
										onClick={() => props.onRemove(member)}
									>
										Remove
									</button>
								</Show>
							</td>
						</tr>
					);
				}}
			</For>
		</tbody>
	</Table>
);

const SingleSignOn = (props: { organization: JsonOrganization }) => {
	const domains = () => props.organization.sso ?? [];
	return (
		<section class="setcard" aria-labelledby="sec-sso">
			<div class="setcard-head">
				<div class="toolbar">
					<h2 class="seclabel" id="sec-sso">
						Single sign-on
					</h2>
					<span class="muted sm">
						{domains().length === 0
							? "Available on Enterprise"
							: domains().length === 1
								? "1 domain"
								: `${domains().length} domains`}
					</span>
				</div>
				<Show when={domains().length === 0}>
					<span class="spacer" />
					<a
						class="ui-button"
						data-variant="secondary"
						data-size="sm"
						href="mailto:enterprise@bencher.dev"
					>
						Contact us
					</a>
				</Show>
			</div>
			<Show when={domains().length}>
				<div class="setcard-body">
					<For each={domains()}>
						{(sso) => (
							<div class="toolbar">
								<Globe />
								<b class="mono">{sso.domain}</b>
								<span class="spacer" />
								<span class="muted sm">{added(sso.created)}</span>
							</div>
						)}
					</For>
				</div>
			</Show>
		</section>
	);
};

const Globe = () => (
	<svg
		class="ui-icon muted"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="9" />
		<path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" />
	</svg>
);
