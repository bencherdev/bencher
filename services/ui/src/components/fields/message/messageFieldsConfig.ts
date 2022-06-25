import validateUsername from "../validators/validateUsername";
import validator from "validator";

const messageFieldsConfig = {
    username: {
        label: "Your Name",
        type: "text",
        placeholder: "Full Name",
        icon: "fas fa-user",
        help: "Must be longer than four characters using only: letters, apostrophes, periods, commas, and dashes",
        validate: validateUsername
    },
    email: {
        label: "Your Email",
        type: "email",
        placeholder: "email@example.com",
        icon: "fas fa-envelope",
        help: "Must be a valid email you have access to",
        validate: validator.isEmail
    }
}

export default messageFieldsConfig;