import { Story, Meta } from "@storybook/html";
import { AuthForm, Props } from "./AuthForm";

export default {
  title: "Auth/AuthForm",
  argTypes: {
    kind: {
      table: {
        type: {
          signup: "signup",
          login: "login",
        },
      },
      control: "string",
    },
    handleTitle: {
      control: "function",
    },
  },
} as Meta;

const Template: Story<Props> = (args: Props) => <AuthForm {...args} />;

const handleTitle = (new_title) => {};

export const SignupForm = Template.bind({});
SignupForm.args = { kind: "signup", handleTitle: handleTitle };

export const LoginForm = Template.bind({});
LoginForm.args = { kind: "signup", handleTitle: handleTitle };
