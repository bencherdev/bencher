const AuthLogoutPage = (props: {
  handleTitle: Function;
  handleRedirect: Function;
  removeUser: Function;
}) => {
  props.handleTitle("Log out");
  props.removeUser();
  props.handleRedirect("/auth/login");
  return <></>;
};

export default AuthLogoutPage;
