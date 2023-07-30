import { BENCHER_GITHUB_URL } from "../../util/ext";

const Footer = () => (<footer class="footer" style="margin-top:64rem;">
	<div class="container">
		<div class="content">
			<nav class="level">
				<div class="level-left">
					<div class="level-item has-text-centered">
						<p>
							<a href="/legal/terms-of-use">Terms of Use</a>
						</p>
					</div>
					<div class="level-item has-text-centered">
						<p>
							<a href="/legal/privacy">Privacy Policy</a>
						</p>
					</div>
					<div class="level-item has-text-centered">
						<p>
							<a href="/legal/license">License Agreement</a>
						</p>
					</div>
				</div>
			</nav>
		</div>
		<div class="columns is-mobile">
			<div class="column">
				<div class="content">
					<p>Bencher - Continuous Benchmarking</p>
				</div>
			</div>
			<div class="column">
				<nav class="level is-mobile">
					<div class="level-item has-text-centered">
						<a
							class="navbar-item"
							href={BENCHER_GITHUB_URL}
							target="_blank"
							aria-label="GitHub"
							rel="noreferrer"
						>
							<span class="icon has-text-primary">
								<i class="fab fa-github fa-2x" aria-hidden="true" />
							</span>
						</a>
					</div>
				</nav>
			</div>
		</div>
		<div class="content">
			<p>© {new Date().getFullYear()} Bencher</p>
		</div>
	</div>
</footer>
);

export default Footer;
