import validateUsername from "../../validators/validateUsername";
import validator from "validator";

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
    validate: validator.isEmail,
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
        <a href="/legal/terms" target="_blank">
          terms and conditions
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
};

export default userFieldsConfig;
