import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import Skeleton from "@bencherdev/ui/Skeleton";
import Table from "@bencherdev/ui/Table";
import TextInput from "@bencherdev/ui/TextInput";
import Truncate from "@bencherdev/ui/Truncate";
import {
	For,
	Match,
	Switch,
	createEffect,
	createSignal,
	onCleanup,
} from "solid-js";
import type { JsonUser } from "../../types/bencher";
import { useAccount } from "../account/context";
import { ApiError } from "../api";
import { userCount, userList } from "../account/users";
import type { Paged } from "../list/paged";
import { MoreRows, Spacer } from "../list/TableRows";
import { useVirtualRows } from "../list/virtual";
import { profilePath } from "../paths";
import { Frame } from "./Profile";

/** How long typing rests before the list asks for what it matches. */
const SEARCH_DELAY = 250;

/** Every user on the server, for a server admin, each leading to their Profile. */
const Users = () => {
	const { api, reader, keepUser } = useAccount();
	const [typed, setTyped] = createSignal("");
	const [search, setSearch] = createSignal("");
	let timer: ReturnType<typeof setTimeout> | undefined;
	onCleanup(() => clearTimeout(timer));
	const type = (text: string) => {
		setTyped(text);
		clearTimeout(timer);
		timer = setTimeout(() => setSearch(text.trim()), SEARCH_DELAY);
	};
	const users = userList(api, search);
	// The API refuses the list to anyone it no longer takes for a server admin, whom the hint follows.
	createEffect(() => {
		const error = users.first().error;
		if (error instanceof ApiError && error.status === 403 && reader().admin) {
			keepUser({ ...reader(), admin: false });
		}
	});
	return (
		<Frame user={reader().slug} section="users">
			<div class="sethead">
				<div class="ph-title">
					<div class="crumb-line">
						Account<span aria-hidden="true">/</span>
					</div>
					<Heading level={1} size="xl">
						Users
					</Heading>
					<p class="ph-sub">
						{users.first().data ? userCount(users.total() ?? 0) : " "}
					</p>
				</div>
				<div class="ph-actions">
					<TextInput
						type="search"
						size="sm"
						placeholder="Search users"
						aria-label="Search users"
						value={typed()}
						onInput={(event) => type(event.currentTarget.value)}
					/>
				</div>
			</div>
			<Switch fallback={<Skeleton size="card" />}>
				<Match when={users.first().data}>
					<UserTable users={users} reader={reader().uuid} />
				</Match>
				<Match when={users.first().error}>
					<Banner status="error" role="alert" class="load-error">
						<span class="grow">Users did not load</span>
						<Button size="sm" onClick={() => users.first().refetch()}>
							Retry
						</Button>
					</Banner>
				</Match>
			</Switch>
		</Frame>
	);
};

export default Users;

const UserTable = (props: { users: Paged<JsonUser>; reader: string }) => {
	const rows = useVirtualRows({
		rows: props.users.rows,
		onRange: props.users.onRange,
	});
	return (
		<Table
			fold={369}
			aria-label="Users"
			aria-rowcount={(props.users.total() ?? 0) + 1}
			onLines={rows.onLines}
		>
			<thead>
				<tr aria-rowindex={1}>
					<th scope="col">Name</th>
					<th scope="col">Email</th>
				</tr>
			</thead>
			<tbody ref={rows.body}>
				<Spacer height={rows.above()} columns={2} />
				<For each={rows.shown()}>
					{(user, index) => (
						<tr class="rowlink" aria-rowindex={rows.start() + index() + 2}>
							<td data-fold="l1" class="ui-table-name">
								<span class="ui-parts">
									<a
										class="namelink rowlink-name"
										href={profilePath(user.slug)}
									>
										<Truncate text={user.name} />
									</a>
									<For
										each={[
											user.uuid === props.reader && "you",
											user.admin && "admin",
											user.locked && "locked",
										].filter((mark) => mark !== false)}
									>
										{(mark) => (
											<>
												{" "}
												<span class="muted sm" data-fixed>
													{mark}
												</span>
											</>
										)}
									</For>
								</span>
							</td>
							<td data-fold="l2" class="muted ui-table-name">
								<span class="ui-parts">
									<Truncate text={user.email} reveal />
								</span>
							</td>
						</tr>
					)}
				</For>
				<Spacer height={rows.below()} columns={2} />
				<MoreRows
					columns={2}
					loading={props.users.loading()}
					onRetry={props.users.retry()}
					failed="These users"
				/>
			</tbody>
		</Table>
	);
};
