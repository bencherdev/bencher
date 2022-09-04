export const LOCAL_USER_KEY: string = "USER_KEY";

export const getToken = () =>
  JSON.parse(window.localStorage.getItem(LOCAL_USER_KEY))?.token;
