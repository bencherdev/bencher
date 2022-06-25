import { Link } from "solid-app-router";

import { authForms } from "./authForms";
import { AuthForm } from "./AuthForm";

const SIGNUP = "signup";
const LOGIN = "login";

export const AuthFormPage = (props: { kind: "signup" | "login" }) => {
  return (
    <section class="section">
      <div class="container">
        <div class="columns is-centered">
          <div class="column is-two-fifths">
            <h2 class="title">
              <span>{authForms[props?.kind]?.heading}</span>
            </h2>

            <AuthForm kind={props?.kind} />

            <hr />

            <p class="has-text-centered">
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
      </div>
    </section>
  );
};
