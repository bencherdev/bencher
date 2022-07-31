import { createSignal, createEffect, Accessor } from "solid-js";
import axios from "axios";

import SiteField from "../site/fields/SiteField";
import userFieldsConfig from "../fields/user/userFieldsConfig";
import authForms from "./authForms";
import { JsonSignup, JsonLogin } from "bencher_json";
import { Field } from "../site/console/console";

const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;

export interface Props {
  kind: "signup" | "login";
  handleTitle: Function;
  handleRedirect: Function;
  user: Accessor<JsonSignup>;
  handleUser: Function;
  handleNotification: Function;
}

export const AuthForm = (props: Props) => {
  const [form, setForm] = createSignal(initForm());

  createEffect(() => {
    handleFormValid();
  });

  props.handleTitle(authForms[props.kind]?.title, false);

  const handleField = (key, value, valid) => {
    setForm({
      ...form(),
      [key]: {
        value: value,
        valid: valid,
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
      if (props.kind === "login") {
        return true;
      }
      if (
        props.kind === "signup" &&
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
    let json_data: JsonSignup | JsonLogin;
    let notification: string;
    if (props.kind === "signup") {
      const signup_form = form();
      json_data = {
        name: signup_form.username.value,
        slug: null,
        email: signup_form.email.value,
        free: null,
      };
      notification = "Successful signup!";
    } else if (props.kind === "login") {
      const login_form = form();
      json_data = {
        email: login_form.email.value,
        free: null,
      };
      notification = "Welcome back!";
    }
    fetchData(json_data)
      .then((resp) => {
        props.handleUser(resp.data);
        props.handleNotification({ status: "ok", text: notification });
        props.handleRedirect("/console");
      })
      .catch((e) => {
        props.handleNotification({
          status: "error",
          text: `Failed to ${props.kind}: ${e}`,
        });
      });
    handleFormSubmitting(false);
  };

  const handleFormSubmitting = (submitting) => {
    setForm({ ...form(), submitting: submitting });
  };

  const request_config = (data: JsonSignup | JsonLogin) => {
    return {
      url: `${BENCHER_API_URL}/v0/auth/${props.kind}`,
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        // Only use with explicit CORS
        // Authorization: `Bearer ${window.localStorage.authToken}`
      },
      data: data,
    };
  };

  const fetchData = async (auth_json: JsonSignup | JsonLogin) => {
    try {
      const config = request_config(auth_json);
      let resp = await axios(config);
      return resp;
    } catch (error) {
      console.error(error);
    }
  };

  return (
    <form class="box">
      {props.kind === "signup" && (
        <SiteField
          kind={Field.INPUT}
          fieldKey="username"
          label={true}
          value={form()?.username?.value}
          valid={form()?.username?.valid}
          config={userFieldsConfig.username}
          handleField={handleField}
        />
      )}

      <SiteField
        kind={Field.INPUT}
        fieldKey="email"
        label={true}
        value={form()?.email?.value}
        valid={form()?.email?.valid}
        config={userFieldsConfig.email}
        handleField={handleField}
      />

      <br />

      {props.kind == "signup" &&
        form()?.username?.valid &&
        form()?.email?.valid && (
          <SiteField
            kind={Field.CHECKBOX}
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
