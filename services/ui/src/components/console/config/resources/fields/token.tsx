import { validate_string, validate_u32 } from "../../../../site/util";
import { is_valid_non_empty } from "bencher_valid";

const TOKEN_FIELDS = {
  name: {
    type: "text",
    placeholder: "Token Name",
    icon: "fas fa-stroopwafel",
    help: "Must be a non-empty string",
    validate: (input) => validate_string(input, is_valid_non_empty),
  },
  ttl: {
    type: "number",
    placeholder: "525600",
    icon: "fas fa-stopwatch",
    help: "Must be an integer greater than zero",
    validate: validate_u32,
  },
};

export default TOKEN_FIELDS;
