import {
  is_valid_email,
  is_valid_user_name,
  is_valid_jwt,
} from "bencher_valid";

const authFieldsConfig = {
  username: {
    label: "Name",
    type: "text",
    placeholder: "Full Name",
    icon: "fas fa-user",
    help: "Must be at least four characters using only: letters, numbers, spaces, apostrophes, periods, commas, and dashes",
    validate: is_valid_user_name,
  },
  email: {
    label: "Email",
    type: "email",
    placeholder: "email@example.com",
    icon: "fas fa-envelope",
    help: "Must be a valid email you have access to",
    validate: is_valid_email,
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
  token: {
    label: "Token",
    type: "text",
    placeholder: "jwt_header.jwt_payload.jwt_verify_signature",
    icon: "fas fa-key",
    help: "Must be a valid JWT (JSON Web Token)",
    validate: is_valid_jwt,
  },
};

export default authFieldsConfig;
