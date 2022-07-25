import { Link, Navigate } from "solid-app-router";
import { createSignal, createEffect, Accessor } from "solid-js";
import { JsonSignup } from "bencher_json";

import authForms from "./authForms";
import { AuthForm } from "./AuthForm";

const SIGNUP = "signup";
const LOGIN = "login";

const AuthLogoutPage = (props: {
  handleTitle: Function;
  handleRedirect: Function;
  handleUser: Function;
}) => {
  props.handleTitle("Log out");
  props.handleUser();
  props.handleRedirect("/");
  return <></>;
};

export default AuthLogoutPage;
