import type { Params } from "astro";
import { Show, createSignal } from "solid-js";
import type { JsonAuthUser } from "../../types/bencher";
import { authUser } from "../../util/auth";
import {
	BENCHER_SITE_URL,
	BENCHER_VERSION,
	BENCHER_WORDMARK,
	isBencherCloud,
} from "../../util/ext";
import ProjectSelect from "./ProjectSelect";
import BENCHER_NAVBAR_ID from "./id";
import { BACK_PARAM, encodePath } from "../../util/url";

export interface Props {
	apiUrl: string;
	params: Params;
}

const ConsoleNavbar = (props: Props) => {
	const user = authUser();

	const [burger, setBurger] = createSignal(false);
	const [dropdown, setDropdown] = createSignal(false);

	return (
		<nav
			id={BENCHER_NAVBAR_ID}
			class="navbar"
			role="navigation"
			aria-label="main navigation"
		>
			<div class="navbar-brand">
				<a class="navbar-item" title="Console Home" href="/console">
					<img src={BENCHER_WORDMARK} width="150" alt="🐰 Bencher" />
				</a>

				<button
					class={`navbar-burger ${burger() && "is-active"}`}
					type="button"
					aria-label="menu"
					aria-expanded="false"
					onClick={(_e) => setBurger(!burger())}
				>
					<span aria-hidden="true" />
					<span aria-hidden="true" />
					<span aria-hidden="true" />
				</button>
			</div>

			<div class={`navbar-menu ${burger() && "is-active"}`}>
				<div class="navbar-start">
					<Show
						when={isBencherCloud()}
						fallback={
							<a
								class="navbar-item"
								href={`${BENCHER_SITE_URL}/docs/`}
								target="_blank"
							>
								Docs
							</a>
						}
					>
						<a class="navbar-item" href="/docs/">
							Docs
						</a>
					</Show>
					<a class="navbar-item" href="/explore/">
						Explore
					</a>
					<Show
						when={user && (props.params?.organization || props.params?.project)}
					>
						<div class="navbar-item">
							<ProjectSelect
								apiUrl={props.apiUrl}
								params={props.params as Params}
								user={user as JsonAuthUser}
							/>
						</div>
					</Show>
				</div>

				<div class="navbar-end">
					<div class="navbar-item">
						<div class="navbar-item">
							<a
								class="button is-outlined"
								href={`/console/users/${
									user?.user?.slug
								}/help/?${BACK_PARAM}=${encodePath()}`}
							>
								<span class="icon has-text-primary">
									<i class="fas fa-life-ring" aria-hidden="true" />
								</span>
								<span>Help</span>
							</a>
						</div>
						<div
							class={`navbar-item has-dropdown is-hoverable ${
								dropdown() && "is-active"
							}`}
						>
							<a class="navbar-link" onClick={(_e) => setDropdown(!dropdown())}>
								{(user?.user?.name ? user?.user?.name : "Account").padStart(
									12,
									"\xa0",
								)}
							</a>
							<div class="navbar-dropdown">
								<a
									class="navbar-item"
									href={`/console/users/${user?.user?.slug}/tokens`}
								>
									Tokens
								</a>
								<a
									class="navbar-item"
									href={`/console/users/${
										user?.user?.slug
									}/settings?${BACK_PARAM}=${encodePath()}`}
								>
									Settings
								</a>
								<hr class="navbar-divider" />
								<div class="navbar-item">
									<a class="button is-light is-fullwidth" href="/auth/logout">
										Log out
									</a>
								</div>
								<hr class="navbar-divider" />
								<div class="navbar-item">v{BENCHER_VERSION}</div>
							</div>
						</div>
						<div class="navbar-item" />
					</div>
				</div>
			</div>
		</nav>
	);
};

export default ConsoleNavbar;
