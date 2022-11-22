import validateUsername from "../../validators/validateUsername";
import validator from "validator";
import { is_valid_email } from "bencher_valid";

const userFieldsConfig = {
  username: {
    label: "Name",
    type: "text",
    placeholder: "Full Name",
    icon: "fas fa-user",
    help: "Must be longer than four characters using only: letters, apostrophes, periods, commas, and dashes",
    validate: validateUsername,
  },
  email: {
    label: "Email",
    type: "email",
    placeholder: "email@example.com",
    icon: "fas fa-envelope",
    help: "Must be a valid email you have access to",
    validate: is_valid_email,
  },
  confirmed: {
    label: "Confirmed",
    type: "switch",
    icon: "far fa-check-circle",
  },
  consent: {
    label: "I Agree",
    type: "checkbox",
    placeholder: (
      <small>
        {" "}
        I agree to the{" "}
        <a href="/legal/terms-of-use" target="_blank">
          terms of use
        </a>
        ,{" "}
        <a href="/legal/privacy" target="_blank">
          privacy policy
        </a>
        , and{" "}
        <a href="/legal/license" target="_blank">
          license agreement
        </a>
        .
      </small>
    ),
  },
  role: {
    label: "Role",
    type: "select",
    icon: "fas fa-user-tag",
  },
  token: {
    label: "Token",
    type: "text",
    placeholder: "jwt_header.jwt_payload.jwt_verify_signature",
    icon: "fas fa-key",
    help: "Must be a valid JWT (JSON Web Token)",
    validate: validator.isJWT,
  },
};

export default userFieldsConfig;
