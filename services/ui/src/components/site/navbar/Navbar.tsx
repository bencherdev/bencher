import { Link } from "solid-app-router";
import { BENCHER_GITHUB_URL, BENCHER_LOGO_URL, BENCHER_VERSION } from "../util";
import ProjectSelect from "./ProjectSelect";

export interface Props {
  user: Function;
  organization_slug: Function;
}

export const Navbar = (props) => {
  return (
    <nav class="navbar" role="navigation" aria-label="main navigation">
      <div class="navbar-brand">
        <Link class="navbar-item" href="/">
          <img src={BENCHER_LOGO_URL} width="152" height="28" />
        </Link>

        <a
          role="button"
          class="navbar-burger"
          aria-label="menu"
          aria-expanded="false"
          data-target="bencherNavbar"
          onClick={() => {
            let toggle = document.querySelector(".navbar-burger");
            let menu = document.querySelector(".navbar-menu");
            toggle.classList.toggle("is-active");
            menu.classList.toggle("is-active");
          }}
        >
          <span aria-hidden="true" />
          <span aria-hidden="true" />
          <span aria-hidden="true" />
        </a>
      </div>

      <div id="navbarBasicExample" class="navbar-menu">
        <div class="navbar-start">
          {!props.user()?.token && (
            <a class="navbar-item" href="/pricing">
              Pricing
            </a>
          )}
          <a class="navbar-item" href="/docs">
            Docs
          </a>
          <a class="navbar-item" href={BENCHER_GITHUB_URL} target="_blank">
            GitHub
          </a>
          {props.user()?.token && props.organization_slug() && (
            <div class="navbar-item">
              <ProjectSelect
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
            <div class="navbar-item">BETA {BENCHER_VERSION}</div>
            <div class="navbar-item" />
            <div class="buttons">
              {props.user()?.token === null ? (
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
