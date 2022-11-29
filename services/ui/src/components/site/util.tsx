import axios from "axios";
import validator from "validator";
import { is_valid_jwt } from "bencher_valid";

import { site_analytics } from "./site_analytics";
import swagger from "../docs/api/swagger.json";

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

export const BENCHER_CALENDLY_URL: string = "https://calendly.com/bencher/demo";

export const BENCHER_LOGO_URL: string =
  "https://s3.amazonaws.com/public.bencher.dev/bencher_navbar.png";

export const BENCHER_USER_KEY: string = "BENCHER_USER";

export const BENCHER_TITLE = "Bencher - Continuous Benchmarking";

export const BENCHER_VERSION = `v${swagger?.info?.version}`;

export const pageTitle = (new_title: string) => {
  if (new_title && new_title.length > 0) {
    const page_title = `${new_title} - Bencher`;
    if (document.title === page_title) {
      return;
    } else {
      document.title = page_title;
    }
  } else {
    document.title = BENCHER_TITLE;
  }

  site_analytics()?.page();
};

export const validate_string = (
  input: string,
  validator: (input: string) => boolean
): boolean => {
  if (typeof input === "string") {
    return validator(input.trim());
  } else {
    return false;
  }
};

export const validate_jwt = (token: string): boolean => {
  return validate_string(token, is_valid_jwt);
};

const HEADERS_CONTENT_TYPE = {
  "Content-Type": "application/json",
};

const get_headers = (token: null | string) => {
  if (is_valid_jwt(token)) {
    return {
      ...HEADERS_CONTENT_TYPE,
      Authorization: `Bearer ${token}`,
    };
  } else {
    return HEADERS_CONTENT_TYPE;
  }
};

export const get_options = (url: string, token: null | string) => {
  return {
    url: url,
    method: "GET",
    headers: get_headers(token),
  };
};

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
    if (!validate_jwt(token)) {
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

export const NOTIFY_KIND_PARAM = "notify_kind";
export const NOTIFY_TEXT_PARAM = "notify_text";

export const isNotifyKind = (kind: any) => {
  switch (parseInt(kind)) {
    case NotifyKind.OK:
    case NotifyKind.ALERT:
    case NotifyKind.ERROR:
      return true;
    default:
      return false;
  }
};

export const isNotifyText = (text: any) =>
  typeof text === "string" && text.length > 0;

export enum NotifyKind {
  OK,
  ALERT,
  ERROR,
}

export const notifyParams = (
  pathname,
  notify_kind: NotifyKind,
  notify_text: string
) => {
  let params = new URLSearchParams(window.location.search);
  params.set(NOTIFY_KIND_PARAM, notify_kind.toString());
  params.set(NOTIFY_TEXT_PARAM, notify_text);
  let params_str = params.toString();
  return `${pathname}?${params_str}`;
};
