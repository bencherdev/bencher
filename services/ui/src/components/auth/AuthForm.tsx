import { createSignal, createEffect } from "solid-js";
import { Link } from "solid-app-router";

export const AuthForm = (props: { kind: "signup" | "login" }) => {
  const [form, setForm] = createSignal(initForm());

  return (
    <form class="box">
      {props?.kind == "signup" && (
        <div class="field">
          <label class="label">Username</label>
          <div class="control">
            <input class="input" type="text" placeholder="user_name" />
          </div>
        </div>
      )}

      <div class="field">
        <label class="label">Email</label>
        <div class="control">
          <input
            class="input"
            type="email"
            placeholder="first.last@example.com"
          />
        </div>
      </div>

      <br />

      {props?.kind == "signup" && (
        <div class="field">
          <label class="checkbox">
            <input type="checkbox" /> I agree to the{" "}
            <Link href="/terms" target="_blank">
              terms and conditions
            </Link>
          </label>
        </div>
      )}

      <button class="button is-primary is-fullwidth">Submit</button>
    </form>
  );
};

const initForm = () => {
  return {
    username: {
      value: "",
      valid: null,
    },
    email: {
      value: "",
      valid: null,
    },
    consent: {
      value: false,
      valid: null,
    },
    valid: false,
    submitting: false,
  };
};
