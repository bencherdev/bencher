import { useNavigate } from "solid-app-router";
import { NotifyKind } from "../site/util";

const AuthLogoutPage = (props: {
  config: any;
  handleTitle: Function;
  removeUser: Function;
  handleNotification: Function;
}) => {
  props.handleTitle(props.config?.title);
  const navigate = useNavigate();

  props.removeUser();
  props.handleNotification(NotifyKind.ALERT, "Goodbye!");
  navigate(props.config?.redirect);

  return <></>;
};

export default AuthLogoutPage;
