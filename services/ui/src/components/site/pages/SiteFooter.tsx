import { Link } from "solid-app-router";
import { BENCHER_GITHUB_URL } from "../util";

const SiteFooter = () => {
  return (
    <footer class="footer">
      <div class="container">
        <div class="content">
          <nav class="level">
            <div class="level-left">
              <div class="level-item has-text-centered">
                <p>
                  <Link href="/legal/terms-of-use">Terms of Use</Link>
                </p>
              </div>
              <div class="level-item has-text-centered">
                <p>
                  <Link href="/legal/privacy-policy">Privacy Policy</Link>
                </p>
              </div>
            </div>
          </nav>
        </div>
        <div class="columns is-mobile">
          <div class="column">
            <div class="content">
              <p>Bencher - Continuous Benchmarking</p>
              <p>© {new Date().getFullYear()} Pompeii LLC</p>
            </div>
          </div>
          <div class="column">
            <nav class="level is-mobile">
              <div class="level-item has-text-centered">
                <a
                  class="navbar-item"
                  href={BENCHER_GITHUB_URL}
                  target="_blank"
                >
                  <span class="icon has-text-primary">
                    <i class="fab fa-github fa-2x" aria-hidden="true" />
                  </span>
                </a>
              </div>
            </nav>
          </div>
        </div>
      </div>
    </footer>
  );
};

export default SiteFooter;
