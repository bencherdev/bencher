import { validate_string } from "../../../site/util";
import { is_valid_non_empty } from "bencher_valid";

const METRIC_KIND_FIELDS = {
  name: {
    type: "text",
    placeholder: "Metric Kind Name",
    icon: "fas fa-shapes",
    help: "Must be non-empty string",
    validate: (input) => validate_string(input, is_valid_non_empty),
  },
  units: {
    type: "text",
    placeholder: "units/time",
    icon: "fas fa-ruler",
    help: "Must be non-empty string",
    validate: (input) => validate_string(input, is_valid_non_empty),
  },
};

export default METRIC_KIND_FIELDS;
