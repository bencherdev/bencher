import { Auth, FormKind } from "./types";

const authConfig = {
  [Auth.SIGNUP]: {
    auth: Auth.SIGNUP,
    title: "Sign up",
    form: {
      kind: FormKind.SIGNUP,
      token: true,
      redirect: "/auth/confirm",
      notification: {
        success: "Sign up successful",
        danger: "Sign up failed",
      },
    },
  },
  [Auth.LOGIN]: {
    auth: Auth.LOGIN,
    title: "Log in",
    form: {
      kind: FormKind.LOGIN,
      token: true,
      redirect: "/auth/confirm",
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
