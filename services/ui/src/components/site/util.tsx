import axios from "axios";
import validator from "validator";

export const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;

export const BENCHER_USER_KEY: string = "BENCHER_USER";

export const getToken = () =>
  JSON.parse(window.localStorage.getItem(BENCHER_USER_KEY))?.token;

export const isAllowedAdmin = async () => {
  return isAllowed(`${BENCHER_API_URL}/v0/admin/allowed`);
};

export const isAllowedOrganization = async (organization: string, permission: string) => {
  return isAllowed(`${BENCHER_API_URL}/v0/organizations/${organization}/allowed/${permission}`);
};

export const isAllowedProject = async (project: string, permission: string) => {
  return isAllowed(`${BENCHER_API_URL}/v0/projects/${project}/allowed/${permission}`);
};

export const isAllowed = async (url: string) => {
  try {
    const token = getToken();
    if (token && !validator.isJWT(token)) {
      return false;
    }
    const options = {
      url: url,
      method: "GET",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${token}`,
      },
    };
    let resp = await axios(options);
    return resp?.data?.allowed;
  } catch (error) {
    console.error(error);
    return false;
  }
};