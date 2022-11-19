import { useNavigate } from "solid-app-router";
import { createEffect } from "solid-js";
import { NotifyKind, notifyParams, pageTitle } from "../site/util";

const AuthLogoutPage = (props: { config: any; removeUser: Function }) => {
  const navigate = useNavigate();

  props.removeUser();
  navigate(notifyParams(props.config?.redirect, NotifyKind.ALERT, "Goodbye!"), {
    replace: true,
  });

  createEffect(() => {
    pageTitle("Logout");
  });

  return <></>;
};

export default AuthLogoutPage;
