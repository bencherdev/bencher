import { NotificationKind } from "../site/util";

const AuthLogoutPage = (props: {
  config: any;
  handleTitle: Function;
  handleRedirect: Function;
  removeUser: Function;
  handleNotification: Function;
}) => {
  props.handleTitle(props.config?.title);
  props.removeUser();
  props.handleNotification(NotificationKind.ALERT, "Goodbye!");
  props.handleRedirect(props.config?.redirect);
  return <></>;
};

export default AuthLogoutPage;
