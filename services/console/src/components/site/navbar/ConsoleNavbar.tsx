import { Show, createSignal } from "solid-js";
import { BENCHER_LOGO_URL, BENCHER_VERSION } from "../../../util/ext";
import { useParams } from "../../../util/url";
import ProjectSelect from "./ProjectSelect";

export interface Props {
	path: string;
}

const ConsoleNavbar = (props: Props) => {
	const pathParams = useParams(props.path);
	const [burger, setBurger] = createSignal(false);

	return (
		<nav class="navbar" role="navigation" aria-label="main navigation">
			<div class="navbar-brand">
				<a class="navbar-item" title="Console Home" href="/console">
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
					<a class="navbar-item" href="/perf">
						Public Projects
					</a>
					<Show when={pathParams.organization_slug} fallback={<></>}>
						<div class="navbar-item">
							<ProjectSelect pathParams={pathParams} />
						</div>
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
							<a class="button is-light" href="/auth/logout">
								Log out
							</a>
						</div>
					</div>
				</div>
			</div>
		</nav>
	);
};

export default ConsoleNavbar;
