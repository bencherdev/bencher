import { createSignal, createEffect, Accessor } from "solid-js";
import axios from "axios";

import SiteField from "../fields/SiteField";
import userFieldsConfig from "../fields/config/user/userFieldsConfig";
import { Field } from "../console/config/types";
import { FormKind } from "./config/types";

const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;

export interface Props {
  config: any;
  handleRedirect: Function;
  user: Function;
  handleUser: Function;
  handleNotification: Function;
}

export const AuthForm = (props: Props) => {
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

  const handleFormValid = () => {
    var valid = validateForm();
    if (valid !== form()?.valid) {
      setForm({ ...form(), valid: valid });
    }
  };

  const validateForm = () => {
    if (form()?.email?.valid) {
      if (props.config?.kind === FormKind.LOGIN) {
        return true;
      }
      if (
        props.config?.kind === FormKind.SIGNUP &&
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
    let json_data;
    if (props.config?.kind === FormKind.SIGNUP) {
      const signup_form = form();
      json_data = {
        name: signup_form.username.value,
        slug: null,
        email: signup_form.email.value,
        free: null,
      };
    } else if (props.config?.kind === FormKind.LOGIN) {
      const login_form = form();
      json_data = {
        email: login_form.email.value,
        free: null,
      };
    }
    fetchData(json_data)
      .then((_resp) => {
        props.handleRedirect(props.config?.redirect);
      })
      .catch((e) => {
        props.handleNotification({
          status: "error",
          text: `Failed to ${props.config?.kind}: ${e}`,
        });
      });
    handleFormSubmitting(false);
  };

  const handleFormSubmitting = (submitting) => {
    setForm({ ...form(), submitting: submitting });
  };

  const request_config = (data) => {
    return {
      url: `${BENCHER_API_URL}/v0/auth/${props.config?.kind}`,
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      data: data,
    };
  };

  const fetchData = async (auth_json) => {
    const config = request_config(auth_json);
    let resp = await axios(config);
    return resp;
  };

  return (
    <form class="box">
      {props.config?.kind === FormKind.SIGNUP && (
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

      {props.config?.kind === FormKind.SIGNUP &&
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
