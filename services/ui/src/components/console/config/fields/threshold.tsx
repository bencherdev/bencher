import { BENCHER_API_URL } from "../../../site/util";

const THRESHOLD_FIELDS = {
  branch: {
    icon: "fas fa-code-branch",
    url: (path_params) =>
      `${BENCHER_API_URL()}/v0/projects/${path_params?.project_slug}/branches`,
  },
  testbed: {
    icon: "fas fa-server",
    url: (path_params) =>
      `${BENCHER_API_URL()}/v0/projects/${path_params?.project_slug}/testbeds`,
  },
  metric_kind: {
    icon: "fas fa-shapes",
    url: (path_params) =>
      `${BENCHER_API_URL()}/v0/projects/${
        path_params?.project_slug
      }/metric-kinds`,
  },
};

export default THRESHOLD_FIELDS;
