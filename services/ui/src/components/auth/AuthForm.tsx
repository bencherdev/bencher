import { createSignal, createEffect, createMemo } from "solid-js";
import { Link } from "solid-app-router";

import SiteField from "../site/fields/SiteField";
import userFieldsConfig from "../fields/user/userFieldsConfig";

export const AuthForm = (props: { kind: "signup" | "login" }) => {
  const [form, setForm] = createSignal(initForm());

  createEffect(() => {
    handleFormValid();
  });

  const handleField = (key, value, valid) => {
    setForm({
      ...form(),
      [key]: {
        value: value,
        valid: valid,
      },
    });
  };

  const handleCheckbox = (_event) => {
    let constent = form()?.consent;
    setForm({
      ...form(),
      consent: {
        value: !constent.value,
        valid: constent.valid,
      },
    });
  };

  const handleFormValid = () => {
    var valid = validateForm();
    if (valid !== form()?.valid) {
      setForm({ ...form(), valid: valid });
    }
  };

  const validateForm = () => {
    if (form()?.email?.valid) {
      if (props?.kind === "login") {
        return true;
      }
      if (
        props?.kind === "signup" &&
        form()?.username?.valid &&
        form()?.consent?.value
      ) {
        return true;
      }
    }
    return false;
  };

  const handleAuthFormSubmit = (event) => {
    event.preventDefault();
    handleFormSubmitting(true);
    // TODO send request to backend
    handleFormSubmitting(false);
  };

  const handleFormSubmitting = (submitting) => {
    setForm({ ...form(), submitting: submitting });
  };

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
            <input
              type="checkbox"
              checked={form()?.consent?.value}
              onChange={(event) => handleCheckbox(event)}
            />{" "}
            I agree to the{" "}
            <Link href="/terms" target="_blank">
              terms and conditions
            </Link>
          </label>
        </div>
      )}

      {props?.kind == "signup" && (
        <SiteField
          type="checkbox"
          fieldKey="consent"
          label={false}
          value={form()?.consent?.value}
          valid={form()?.consent?.valid}
          config={userFieldsConfig.consent}
          handleField={handleField}
        />
      )}

      <button
        class="button is-primary is-fullwidth"
        disabled={!form()?.valid || form()?.submitting}
        onClick={(e) => handleAuthFormSubmit(e)}
      >
        Submit
      </button>
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
