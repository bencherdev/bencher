import { BENCHER_API_URL } from "../../../site/util";

const THRESHOLD_FIELDS = {
  branch: {
    icon: "fas fa-code-branch",
    option_key: "name",
    value_key: "uuid",
    url: (path_params) =>
      `${BENCHER_API_URL()}/v0/projects/${path_params?.project_slug}/branches`,
  },
  testbed: {
    icon: "fas fa-server",
    option_key: "name",
    value_key: "uuid",
    url: (path_params) =>
      `${BENCHER_API_URL()}/v0/projects/${path_params?.project_slug}/testbeds`,
  },
  metric_kind: {
    icon: "fas fa-shapes",
    option_key: "name",
    value_key: "uuid",
    url: (path_params) =>
      `${BENCHER_API_URL()}/v0/projects/${
        path_params?.project_slug
      }/metric-kinds`,
  },
  test: {
    icon: "fas fa-vial",
  },
};

export default THRESHOLD_FIELDS;
