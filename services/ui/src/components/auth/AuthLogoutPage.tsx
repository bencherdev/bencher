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
