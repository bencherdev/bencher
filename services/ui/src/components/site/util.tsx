import axios from "axios";
import validator from "validator";

// Either supply `VITE_BENCHER_API_URL` at build time,
// or default to the current protocol and hostname at port `61016`.
// If another endpoint is required, then the UI will need to be re-bundled.
export const BENCHER_API_URL: () => string = () => {
  const api_url = import.meta.env.VITE_BENCHER_API_URL;
  if (api_url) {
    return api_url;
  } else {
    const location = window.location;
    return location.protocol + "//" + location.hostname + ":61016";
  }
};

export const BENCHER_GITHUB_URL: string =
  "https://github.com/bencherdev/bencher";

export const BENCHER_USER_KEY: string = "BENCHER_USER";

export const getToken = () =>
  JSON.parse(window.localStorage.getItem(BENCHER_USER_KEY))?.token;

export const isAllowedAdmin = async () => {
  return isAllowed(`${BENCHER_API_URL()}/v0/admin/allowed`);
};

export enum OrganizationPermission {
  VIEW = "view",
  CREATE = "create",
  EDIT = "edit",
  DELETE = "delete",
  MANAGE = "manage",
  VIEW_ROLE = "view_role",
  CREATE_ROLE = "create_role",
  EDIT_ROLE = "edit_role",
  DELETE_ROLE = "delete_role",
}

export const isAllowedOrganization = async (
  path_params,
  permission: OrganizationPermission
) => {
  return isAllowed(
    `${BENCHER_API_URL()}/v0/organizations/${
      path_params?.organization_slug
    }/allowed/${permission}`
  );
};

export enum ProjectPermission {
  VIEW = "view",
  CREATE = "create",
  EDIT = "edit",
  DELETE = "delete",
  MANAGE = "manage",
  VIEW_ROLE = "view_role",
  CREATE_ROLE = "create_role",
  EDIT_ROLE = "edit_role",
  DELETE_ROLE = "delete_role",
}

export const isAllowedProject = async (
  path_params,
  permission: ProjectPermission
) => {
  return isAllowed(
    `${BENCHER_API_URL()}/v0/projects/${
      path_params?.project_slug
    }/allowed/${permission}`
  );
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

export enum NotificationKind {
  OK,
  ALERT,
  ERROR,
}
