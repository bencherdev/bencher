import validateName from "../../validators/validateName";
import validateSlug from "../../validators/validateSlug";
import validator from "validator";

const memberFieldsConfig = {
  name: {
    label: "Name",
    type: "text",
    placeholder: "Member Name",
    icon: "fas fa-user",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  slug: {
    label: "Slug",
    type: "text",
    placeholder: "Member Slug",
    icon: "fas fa-exclamation-triangle",
    help: "Must be at least four characters or longer.",
    validate: validateSlug,
  },
  email: {
    label: "Email",
    type: "email",
    placeholder: "email@example.com",
    icon: "fas fa-envelope",
    help: "Must be a valid email you have access to",
    validate: validator.isEmail,
  },
  organization: {
    label: "Organization",
    type: "text",
    placeholder: "Organization Slug",
    icon: "fas fa-project-diagram",
  },
  role: {
    label: "Role",
    type: "select",
    icon: "fas fa-user-tag",
  },
};

export default memberFieldsConfig;
