import { createMemo, createSignal, Show } from "solid-js";
import {
	BENCHER_GITHUB_URL,
	BENCHER_LOGO_URL,
	BENCHER_VERSION,
} from "../../../util/ext";
// import {
// 	BENCHER_GITHUB_URL,
// 	BENCHER_LOGO_URL,
// 	BENCHER_VERSION,
// 	validate_jwt,
// } from "../util";
// import ProjectSelect from "./ProjectSelect";

export interface Props {
	slot: string;
	user: Function;
	organization_slug: Function;
}

const Navbar = (props: Props) => {
	// const is_valid_jwt = createMemo(() => validate_jwt(props.user.token));
	const [burger, setBurger] = createSignal(false);

	const is_valid_jwt = () => false;

	return (
		<nav class="navbar" role="navigation" aria-label="main navigation">
			<div class="navbar-brand">
				<a
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
				</a>

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
					<a class="navbar-item" href="/docs">
						Docs
					</a>
					<Show
						when={is_valid_jwt()}
						fallback={
							<>
								<a class="navbar-item" href="/perf">
									Projects
								</a>
								<a class="navbar-item" href="/pricing">
									Pricing
								</a>
								<a
									class="navbar-item"
									href={BENCHER_GITHUB_URL}
									target="_blank"
									rel="noreferrer"
								>
									GitHub
								</a>
							</>
						}
					>
						<a class="navbar-item" href="/perf">
							Public Projects
						</a>
						<Show when={props.organization_slug()} fallback={<></>}>
							<div class="navbar-item">
								{/* <ProjectSelect
									user={props.user}
									organization_slug={props.organization_slug}
									project_slug={props?.project_slug}
									handleRedirect={props?.handleRedirect}
									handleProjectSlug={props?.handleProjectSlug}
								/> */}
							</div>
						</Show>
					</Show>
				</div>

				<div class="navbar-end">
					<div class="navbar-item">
						<div class="navbar-item">BETA v{BENCHER_VERSION}</div>
						<div class="navbar-item" />
						<div class="navbar-item">
							<a class="button is-outlined" href="/help">
								<span class="icon has-text-primary">
									<i class="fas fa-life-ring" aria-hidden="true" />
								</span>
								<span>Help</span>
							</a>
						</div>
						<div class="navbar-item" />
						<div class="buttons">
							<Show
								when={is_valid_jwt()}
								fallback={
									<>
										<a class="button is-light" href="/auth/login">
											Log in
										</a>
										<a class="button is-primary" href="/auth/signup">
											<strong>Sign up</strong>
										</a>
									</>
								}
							>
								<a class="button is-light" href="/auth/logout">
									Log out
								</a>
							</Show>
						</div>
					</div>
				</div>
			</div>
		</nav>
	);
};

export default Navbar;
