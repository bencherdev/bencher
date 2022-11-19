import { useNavigate } from "solid-app-router";
import { createEffect } from "solid-js";
import { NotifyKind, pageTitle } from "../site/util";

const AuthLogoutPage = (props: {
  config: any;
  removeUser: Function;
  handleNotification: Function;
}) => {
  const navigate = useNavigate();

  props.removeUser();
  props.handleNotification(NotifyKind.ALERT, "Goodbye!");
  navigate(props.config?.redirect, { replace: true });

  createEffect(() => {
    pageTitle("Logout");
  });

  return <></>;
};

export default AuthLogoutPage;
