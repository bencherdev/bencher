import { Show, createSignal } from "solid-js";
import type { AccountMenuProps } from "./AccountMenu";
import { lazyView } from "./lazy";

const menu = lazyView(() => import("./AccountMenu"));

/** The avatar, which opens the account menu; the menu's code loads on first intent. */
const Avatar = (
	props: Omit<AccountMenuProps, "user" | "anchor" | "onClose"> & {
		/** Unknown to the server's render. */
		user?: AccountMenuProps["user"] | undefined;
	},
) => {
	const [view, setView] = createSignal(menu.loaded());
	const [open, setOpen] = createSignal(false);
	let button: HTMLButtonElement | undefined;
	return (
		<Show when={props.user} fallback={<span class="avatar" />}>
			{(user) => (
				<div class="account">
					<button
						ref={button}
						type="button"
						class="avatar"
						aria-label="Account"
						aria-haspopup="menu"
						aria-expanded={open()}
						onPointerEnter={menu.preload}
						onFocus={menu.preload}
						onClick={() => {
							if (open()) {
								setOpen(false);
								return;
							}
							menu
								.load()
								.then((view) => {
									setView(() => view);
									setOpen(true);
								})
								.catch(() => {});
						}}
					/>
					<Show when={open() ? view() : undefined}>
						{(view) => {
							const AccountMenu = view();
							return (
								<AccountMenu
									{...props}
									user={user()}
									anchor={button}
									onClose={() => setOpen(false)}
								/>
							);
						}}
					</Show>
				</div>
			)}
		</Show>
	);
};

export default Avatar;
