import { Link, Navigate } from "solid-app-router";
import { createSignal, createEffect, Accessor } from "solid-js";

import authForms from "./authForms";
import { AuthForm } from "./AuthForm";

const SIGNUP = "signup";
const LOGIN = "login";

const AuthLogoutPage = (props: {
  config: any;
  handleTitle: Function;
  handleRedirect: Function;
  removeUser: Function;
}) => {
  props.handleTitle(props.config?.title);
  props.removeUser();
  props.handleRedirect(props.config?.redirect);
  return <></>;
};

export default AuthLogoutPage;
