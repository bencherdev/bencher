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
    console.log(`${key} ${value} ${valid}`);
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
      {props.kind === "signup" && (
        <SiteField
          type="input"
          fieldKey="username"
          label={true}
          value={form()?.username?.value}
          valid={form()?.username?.valid}
          config={userFieldsConfig.username}
          handleField={handleField}
        />
      )}

      <SiteField
        type="input"
        fieldKey="email"
        label={true}
        value={form()?.email?.value}
        valid={form()?.email?.valid}
        config={userFieldsConfig.email}
        handleField={handleField}
      />

      <br />

      {props?.kind == "signup" &&
        form()?.username?.valid &&
        form()?.email?.valid && (
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
