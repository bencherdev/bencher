export const BENCHER_USER_KEY: string = "BENCHER_USER";

export const getToken = () =>
  JSON.parse(window.localStorage.getItem(BENCHER_USER_KEY))?.token;

