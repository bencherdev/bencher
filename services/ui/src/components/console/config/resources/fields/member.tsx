import {
  is_valid_email,
  is_valid_slug,
  is_valid_user_name,
} from "bencher_valid";
import { validate_string } from "../../../../site/util";

const MEMBER_FIELDS = {
  name: {
    type: "text",
    placeholder: "Member Name",
    icon: "fas fa-user",
    help: "May only use: letters, numbers, contained spaces, apostrophes, periods, commas, and dashes",
    validate: (input) => validate_string(input, is_valid_user_name),
  },
  slug: {
    type: "text",
    placeholder: "Member Slug",
    icon: "fas fa-exclamation-triangle",
    help: "Must be at least four characters or longer",
    validate: (input) => validate_string(input, is_valid_slug),
  },
  email: {
    type: "email",
    placeholder: "email@example.com",
    icon: "fas fa-envelope",
    help: "Must be a valid email address",
    validate: (input) => validate_string(input, is_valid_email),
  },
  role: {
    icon: "fas fa-user-tag",
  },
};

export default MEMBER_FIELDS;
