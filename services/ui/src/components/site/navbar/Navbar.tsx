import { Link } from "solid-app-router";
import { createMemo, createSignal } from "solid-js";
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
				<Link class="navbar-item" href="/" aria-label="home">
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
					onClick={() => setBurger(!burger())}
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
					{is_valid_jwt() ? (
						<a class="navbar-item" href="/perf">
							Public Projects
						</a>
					) : (
						<a class="navbar-item" href="/perf">
							Projects
						</a>
					)}
					{!is_valid_jwt() && (
						<a class="navbar-item" href="/pricing">
							Pricing
						</a>
					)}
					{!is_valid_jwt() && (
						<a
							class="navbar-item"
							href={BENCHER_GITHUB_URL}
							target="_blank"
							rel="noreferrer"
						>
							GitHub
						</a>
					)}
					{is_valid_jwt() && props.organization_slug() && (
						<div class="navbar-item">
							<ProjectSelect
								user={props.user}
								organization_slug={props.organization_slug}
								project_slug={props?.project_slug}
								handleRedirect={props?.handleRedirect}
								handleProjectSlug={props?.handleProjectSlug}
							/>
						</div>
					)}
				</div>

				<div class="navbar-end">
					<div class="navbar-item">
						<div class="navbar-item">BETA v{BENCHER_VERSION}</div>
						<div class="navbar-item" />
						<div class="buttons">
							{props.user?.token === null ? (
								<>
									<Link class="button is-light" href="/auth/login">
										Log in
									</Link>
									<Link class="button is-primary" href="/auth/signup">
										<strong>Sign up</strong>
									</Link>
								</>
							) : (
								<Link class="button is-light" href="/auth/logout">
									Log out
								</Link>
							)}
						</div>
					</div>
				</div>
			</div>
		</nav>
	);
};
