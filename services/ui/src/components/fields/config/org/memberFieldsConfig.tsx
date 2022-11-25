import { is_valid_email } from "bencher_valid";

import validateName from "../../validators/validateName";
import validateSlug from "../../validators/validateSlug";

const memberFieldsConfig = {
  name: {
    type: "text",
    placeholder: "Member Name",
    icon: "fas fa-user",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  slug: {
    type: "text",
    placeholder: "Member Slug",
    icon: "fas fa-exclamation-triangle",
    help: "Must be at least four characters or longer.",
    validate: validateSlug,
  },
  email: {
    type: "email",
    placeholder: "email@example.com",
    icon: "fas fa-envelope",
    help: "Must be a valid email address",
    validate: is_valid_email,
  },
  role: {
    type: "select",
    icon: "fas fa-user-tag",
  },
};

export default memberFieldsConfig;
