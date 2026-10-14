import Icon from "@bencherdev/ui/Icon";
import Menu from "@bencherdev/ui/Menu";
import Sheet from "@bencherdev/ui/Sheet";
import { QueryClientContext } from "@tanstack/solid-query";
import { type JSX, Show, createSignal, useContext } from "solid-js";
import { accountPath } from "../account/paths";
import { ORGANIZATIONS_PATH, profilePath } from "../paths";
import { NARROW } from "../plot/narrow";
import { ProjectContext } from "../project";
import { consoleUserQuery } from "../queries";
import { lazyView } from "./lazy";

const switcher = lazyView(() => import("../switcher/Switcher"));

export interface AccountMenuProps {
	user: { name: string; slug: string; email: string };
	signOut?: ((back?: boolean) => void) | undefined;
	/** The project on screen, which a phone's menu switches from. */
	slug?: string | undefined;
	project?: string | undefined;
	organization?: { name: string; uuid: string } | undefined;
	/** The signed in reader, whose projects the switcher lists. */
	reader?: string | undefined;
	anchor: HTMLElement | undefined;
	onClose: () => void;
}

/** The reader's account entries; on a phone, with no crumb, it switches projects too. */
const AccountMenu = (props: AccountMenuProps) => {
	const client = useContext(QueryClientContext);
	const context = useContext(ProjectContext);
	const here =
		matchMedia(NARROW).matches && props.slug && props.reader
			? { slug: props.slug, reader: props.reader }
			: undefined;
	const [Switcher, setSwitcher] =
		createSignal<Awaited<ReturnType<typeof switcher.load>>>();
	// On a phone the switcher's code and the reader's projects start with the menu.
	if (here && client && context) {
		switcher.preload();
		client()
			.query(consoleUserQuery(context.api, here.reader))
			.catch(() => {});
	}
	const close = (refocus: boolean) => {
		props.onClose();
		if (refocus) {
			props.anchor?.focus();
		}
	};
	return (
		<Show
			when={Switcher()}
			fallback={
				<Menu
					label="Account"
					anchor={props.anchor}
					align="end"
					class="account-menu"
					onClose={props.onClose}
					header={
						<>
							<div class="menu-who">
								<b>{props.user.name}</b>
								<span>{props.user.email}</span>
							</div>
							<hr class="ui-menu-sep" />
						</>
					}
				>
					<Show when={here}>
						{(at) => (
							<>
								<Item
									onClick={() =>
										switcher
											.load()
											.then((view) => setSwitcher(() => view))
											.catch(() => {})
									}
								>
									<Glyph d="M7 7h11l-3-3M17 17H6l3 3" />
									Switch project
									<span class="end">
										{props.project ?? at().slug}
										<Glyph d="M9 6l6 6-6 6" />
									</span>
								</Item>
								<hr class="ui-menu-sep" />
							</>
						)}
					</Show>
					<Link href={profilePath(props.user.slug)} onClick={props.onClose}>
						<Icon name="user" />
						Profile
					</Link>
					<Link
						href={accountPath(props.user.slug, "keys")}
						onClick={props.onClose}
					>
						<Icon name="key" />
						Keys
					</Link>
					<Link href={`/console/users/${props.user.slug}/apps`}>
						<Glyph d="M12 22v-5M9 8V2M15 8V2M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8z" />
						Connected Apps
						<span class="end">
							<Glyph d="M7 17L17 7M8 7h9v9" />
						</span>
					</Link>
					<Link href={ORGANIZATIONS_PATH} onClick={props.onClose}>
						<Glyph d="M6 3h12a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zM9 7h1M14 7h1M9 11h1M14 11h1M9 15h1M14 15h1" />
						Organizations
					</Link>
					<hr class="ui-menu-sep" />
					<Link href="/help/">
						<Glyph d="M3 12a9 9 0 1 0 18 0 9 9 0 1 0-18 0zM9.5 9a2.5 2.5 0 0 1 4.9.8c0 1.7-2.4 2.2-2.4 3.7M12 17v.01" />
						Help
						<span class="end">
							<Glyph d="M7 17L17 7M8 7h9v9" />
						</span>
					</Link>
					<hr class="ui-menu-sep" />
					<Item onClick={() => props.signOut?.(false)}>
						<Glyph d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9" />
						Sign out
					</Item>
				</Menu>
			}
		>
			{(view) => {
				const Switcher = view();
				return (
					here && (
						<Sheet
							open
							title="Switch project"
							done="Close"
							onClose={() => close(true)}
						>
							<Switcher
								sheet
								reader={here.reader}
								slug={here.slug}
								organization={props.organization}
								crumb={props.anchor}
								onClose={close}
							/>
						</Sheet>
					)
				);
			}}
		</Show>
	);
};

export default AccountMenu;

const Item = (props: JSX.ButtonHTMLAttributes<HTMLButtonElement>) => (
	<button
		type="button"
		role="menuitem"
		tabindex={-1}
		class="ui-menu-item"
		{...props}
	/>
);

const Link = (props: JSX.AnchorHTMLAttributes<HTMLAnchorElement>) => (
	<a role="menuitem" tabindex={-1} class="ui-menu-item" {...props} />
);

const Glyph = (props: { d: string }) => (
	<svg
		class="ui-icon"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<path d={props.d} />
	</svg>
);
