import { Link } from "solid-app-router";

import { authForms } from "./authForms";

const SIGNUP = "signup";
const LOGIN = "login";

export const AuthFormPage = (props: { kind: "signup" | "login" }) => {
  return (
    <section class="section">
      <div class="container">
        <div class="columns is-centered is-mobile">
          <div class="column is-two-fifths">
            <h2>
              <span>{authForms[props?.kind]?.heading}</span>
            </h2>
            <hr />

            <nav class="level">
              <div class="level-left">
                <div class="level-item has-text-centered">
                  <p>
                    <small>
                      switch to{" "}
                      {props?.kind === SIGNUP && (
                        <Link href="/auth/login">log in</Link>
                      )}
                      {props?.kind === LOGIN && (
                        <Link href="/auth/signup">sign up</Link>
                      )}
                    </small>
                  </p>
                </div>
              </div>
            </nav>
          </div>
        </div>
      </div>
    </section>
  );
};
