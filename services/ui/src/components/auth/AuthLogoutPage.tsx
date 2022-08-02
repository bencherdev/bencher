import { Link, Navigate } from "solid-app-router";
import { createSignal, createEffect, Accessor } from "solid-js";

import authForms from "./authForms";
import { AuthForm } from "./AuthForm";

const SIGNUP = "signup";
const LOGIN = "login";

const AuthLogoutPage = (props: {
  handleTitle: Function;
  handleRedirect: Function;
  removeUser: Function;
}) => {
  props.handleTitle("Log out");
  props.removeUser();
  props.handleRedirect("/");
  return <></>;
};

export default AuthLogoutPage;
