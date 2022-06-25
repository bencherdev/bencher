import validateName from "../validators/validateName";
import validateDescription from "../validators/validateDescription";

const contactFieldsConfig = {
    title: {
        label: "Title",
        type: "text",
        placeholder: "Message Title",
        icon: "fas fa-heading",
        help: "Must be longer at least four characters or longer",
        validate: validateName
    },
    message: {
        label: "Message",
        type: "text",
        placeholder: "Tell us what you have to say...",
        icon: "far fa-comment-alt",
        help: "Must be between 25 and 500 characters",
        validate: validateDescription
    }
}

export default contactFieldsConfig;