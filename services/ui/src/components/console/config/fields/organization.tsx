import { validate_string } from "../../../site/util";
import { is_valid_slug, is_valid_non_empty } from "bencher_valid";

const ORGANIZATION_FIELDS = {
  name: {
    label: "Name",
    type: "text",
    placeholder: "Organization Name",
    icon: "fas fa-project-diagram",
    help: "Must be a non-empty string",
    validate: (input) => validate_string(input, is_valid_non_empty),
  },
  slug: {
    label: "Slug",
    type: "text",
    placeholder: "Organization Slug",
    icon: "fas fa-exclamation-triangle",
    help: "Must be a valid slug",
    validate: (input) => validate_string(input, is_valid_slug),
  },
};

export default ORGANIZATION_FIELDS;
