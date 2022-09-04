import { Link, Navigate } from "solid-app-router";
import { createSignal } from "solid-js";
import validator from "validator";

import { AuthForm } from "./AuthForm";
import { Auth } from "./config/types";

const AuthFormPage = (props: {
  config: any;
  handleTitle: Function;
  handleRedirect: Function;
  user: Function;
  handleUser: Function;
  handleNotification: Function;
}) => {
  props.handleTitle(props.config?.title);

  const [authenticated, setAuthenticated] = createSignal(false);

  setInterval(() => {
    if (validator.isJWT(props.user()?.token)) {
      setAuthenticated(true);
    }
  }, 1000);

  return (
    <section class="section">
      <div class="container">
        <div class="columns is-centered">
          <div class="column is-two-fifths">
            {authenticated() && <Navigate href={"/console"} />}
            <h2 class="title">{props.config?.title}</h2>

            <AuthForm
              config={props.config?.form}
              handleRedirect={props.handleRedirect}
              user={props.user}
              handleUser={props.handleUser}
              handleNotification={props.handleNotification}
            />

            <hr />

            <p class="has-text-centered">
              <small>
                switch to{" "}
                {props.config?.auth === Auth.SIGNUP && (
                  <Link href="/auth/login">log in</Link>
                )}
                {props.config?.auth === Auth.LOGIN && (
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

export default AuthFormPage;
