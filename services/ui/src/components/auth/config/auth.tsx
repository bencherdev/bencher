import { Auth } from "./types";

const authConfig = {
  [Auth.SIGNUP]: {
    auth: Auth.SIGNUP,
    heading: "Sign up",
    form: {
      token: true,
      redirect: "/dashboard",
      notification: {
        success: "Sign up successful",
        danger: "Sign up failed",
      },
    },
  },
  [Auth.LOGIN]: {
    auth: Auth.LOGIN,
    heading: "Log in",
    form: {
      token: true,
      redirect: "/dashboard",
      notification: {
        success: "Log in successful",
        danger: "Log in failed",
      },
    },
  },
  [Auth.CONFIRM]: {
    auth: Auth.CONFIRM,
  },
  [Auth.LOGOUT]: {
    auth: Auth.LOGOUT,
    title: "Log out",
    redirect: "/auth/login",
  },
};

export default authConfig;
