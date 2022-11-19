import { Link, useNavigate, useSearchParams } from "solid-app-router";
import { createEffect, createMemo } from "solid-js";
import validator from "validator";
import { pageTitle } from "../site/util";

import { AuthForm } from "./AuthForm";
import { Auth } from "./config/types";

const INVITE_PARAM = "invite";

const AuthFormPage = (props: {
  config: any;
  pathname: Function;
  user: Function;
  handleUser: Function;
  handleNotification: Function;
}) => {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();

  if (
    searchParams[INVITE_PARAM] &&
    !validator.isJWT(searchParams[INVITE_PARAM].trim())
  ) {
    setSearchParams({ [INVITE_PARAM]: null });
  }

  const invite = createMemo(() =>
    searchParams[INVITE_PARAM] ? searchParams[INVITE_PARAM].trim() : null
  );

  createEffect(() => {
    if (props.user().token && validator.isJWT(props.user().token)) {
      navigate("/console");
    }

    pageTitle(props.config?.title);
  });

  return (
    <section class="section">
      <div class="container">
        <div class="columns is-centered">
          <div class="column is-two-fifths">
            <h2 class="title">{props.config?.title}</h2>

            <AuthForm
              config={props.config?.form}
              pathname={props.pathname}
              user={props.user}
              invite={invite}
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
