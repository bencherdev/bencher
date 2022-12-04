import { validate_string } from "../../../site/util";
import { is_valid_slug, is_valid_non_empty, is_valid_url } from "bencher_valid";

const PROJECT_FIELDS = {
  name: {
    type: "text",
    placeholder: "Project Name",
    icon: "fas fa-project-diagram",
    help: "Must be non-empty string",
    validate: (input) => validate_string(input, is_valid_non_empty),
  },
  slug: {
    type: "text",
    placeholder: "Project Slug",
    icon: "fas fa-exclamation-triangle",
    help: "Must be a valid slug",
    validate: (input) => validate_string(input, is_valid_slug),
  },
  url: {
    type: "text",
    placeholder: "www.example.com",
    icon: "fas fa-link",
    help: "Must be a valid URL",
    validate: (input) => validate_string(input, is_valid_url),
  },
  public: {
    type: "checkbox",
    disabled: true,
  },
};

export default PROJECT_FIELDS;
