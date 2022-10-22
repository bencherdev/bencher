import validateDescription from "../../validators/validateDescription";
import validateName from "../../validators/validateName";
import validateSlug from "../../validators/validateSlug";
import validator from "validator";

const projectFieldsConfig = {
  name: {
    label: "Name",
    type: "text",
    placeholder: "Organization Name",
    icon: "fas fa-project-diagram",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  slug: {
    label: "Slug",
    type: "text",
    placeholder: "Organization Slug",
    icon: "fas fa-exclamation-triangle",
    help: "Must be at least four characters or longer.",
    validate: validateSlug,
  },
};

export default projectFieldsConfig;
