import { Link } from "solid-app-router";
import { createMemo, createSignal, Show } from "solid-js";
import {
	BENCHER_GITHUB_URL,
	BENCHER_LOGO_URL,
	BENCHER_VERSION,
	validate_jwt,
} from "../util";
import ProjectSelect from "./ProjectSelect";

export interface Props {
	user: Function;
	organization_slug: Function;
}

export const Navbar = (props) => {
	const is_valid_jwt = createMemo(() => validate_jwt(props.user.token));
	const [burger, setBurger] = createSignal(false);

	return (
		<nav class="navbar" role="navigation" aria-label="main navigation">
			<div class="navbar-brand">
				<Link
					class="navbar-item"
					title={
						is_valid_jwt()
							? "Console Home"
							: "Bencher - Continuous Benchmarking"
					}
					href="/"
				>
					<img
						src={BENCHER_LOGO_URL}
						width="152"
						height="28"
						alt="🐰 Bencher"
					/>
				</Link>

				<button
					class={`navbar-burger ${burger() && "is-active"}`}
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
					<a class="navbar-item" title="Bencher Docs" href="/docs">
						Docs
					</a>
					<Show
						when={is_valid_jwt()}
						fallback={
							<>
								<a class="navbar-item" title="Bencher Projects" href="/perf">
									Projects
								</a>
								<a class="navbar-item" title="Bencher Pricing" href="/pricing">
									Pricing
								</a>
								<a
									class="navbar-item"
									title="Bencher GitHub Repository"
									href={BENCHER_GITHUB_URL}
									target="_blank"
									rel="noreferrer"
								>
									GitHub
								</a>
							</>
						}
					>
						<a class="navbar-item" title="Public Projects" href="/perf">
							Public Projects
						</a>
						<Show when={props.organization_slug()} fallback={<></>}>
							<div class="navbar-item">
								<ProjectSelect
									user={props.user}
									organization_slug={props.organization_slug}
									project_slug={props?.project_slug}
									handleRedirect={props?.handleRedirect}
									handleProjectSlug={props?.handleProjectSlug}
								/>
							</div>
						</Show>
					</Show>
				</div>

				<div class="navbar-end">
					<div class="navbar-item">
						<div class="navbar-item">BETA v{BENCHER_VERSION}</div>
						<div class="navbar-item" />
						<div class="buttons">
							<Show
								when={is_valid_jwt()}
								fallback={
									<>
										<Link
											class="button is-light"
											title="Bencher Log in"
											href="/auth/login"
										>
											Log in
										</Link>
										<Link
											class="button is-primary"
											title="Bencher Sign up"
											href="/auth/signup"
										>
											<strong>Sign up</strong>
										</Link>
									</>
								}
							>
								<Link
									class="button is-light"
									title="Log out"
									href="/auth/logout"
								>
									Log out
								</Link>
							</Show>
						</div>
					</div>
				</div>
			</div>
		</nav>
	);
};
