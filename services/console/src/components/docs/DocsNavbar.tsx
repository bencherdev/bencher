import { authUser } from "../../util/auth";
import { BENCHER_LOGO_URL } from "../../util/ext";
import AuthOverride from "../auth/AuthOverride";
import BENCHER_NAVBAR_ID from "../site/navbar/id";

const DocsNavbar = () => {
	const user = authUser();

	return (
		<AuthOverride elementId={BENCHER_NAVBAR_ID}>
			<nav
				id={BENCHER_NAVBAR_ID}
				class="navbar"
				role="navigation"
				aria-label="main navigation"
			>
				<div class="navbar-brand">
					<a class="navbar-item" title="Console Home" href="/console">
						<img
							src={BENCHER_LOGO_URL}
							width="152"
							height="28"
							alt="🐰 Bencher"
						/>
					</a>
				</div>

				<div class="navbar-menu is-active">
					<div class="navbar-start">
						<a class="navbar-item" href="/docs">
							Docs
						</a>
						<a class="navbar-item" href="/perf">
							Public Projects
						</a>
					</div>

					<div class="navbar-end">
						<div class="navbar-item">
							<div class="select">
								<select>
									<option>🇬🇧</option>
									<option>🇫🇷</option>
								</select>
							</div>
						</div>
						<div class="navbar-item">
							<a
								class="button is-outlined"
								href={`/console/users/${user?.user?.slug}/help`}
							>
								<span class="icon has-text-primary">
									<i class="fas fa-life-ring" aria-hidden="true" />
								</span>
								<span>Help</span>
							</a>
						</div>
						<div class="navbar-item">
							<a class="button is-outlined" href="/console">
								<span class="icon has-text-primary">
									<i class="fas fa-angle-left" aria-hidden="true" />
								</span>
								<span>Back to Console</span>
							</a>
						</div>
						<div class="navbar-item"></div>
					</div>
				</div>
			</nav>
		</AuthOverride>
	);
};

export default DocsNavbar;
