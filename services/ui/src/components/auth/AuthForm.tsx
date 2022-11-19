import { createSignal, createEffect, Accessor } from "solid-js";
import axios from "axios";
import validator from "validator";

import SiteField from "../fields/SiteField";
import userFieldsConfig from "../fields/config/user/userFieldsConfig";
import { Field } from "../console/config/types";
import { FormKind } from "./config/types";
import {
  BENCHER_API_URL,
  NotifyKind,
  NOTIFY_KIND_PARAM,
  NOTIFY_TEXT_PARAM,
} from "../site/util";
import { useNavigate } from "solid-app-router";

export interface Props {
  config: any;
  pathname: Function;
  user: Function;
  invite: Function;
  handleUser: Function;
  handleNotification: Function;
}

export const AuthForm = (props: Props) => {
  const navigate = useNavigate();
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
    const invite_token = props.invite();
    let invite: string | null;
    if (invite_token && validator.isJWT(invite_token)) {
      invite = invite_token;
    } else {
      invite = null;
    }

    let json_data;
    if (props.config?.kind === FormKind.SIGNUP) {
      const signup_form = form();
      json_data = {
        name: signup_form.username.value,
        slug: null,
        email: signup_form.email.value,
        invite: invite,
      };
    } else if (props.config?.kind === FormKind.LOGIN) {
      const login_form = form();
      json_data = {
        email: login_form.email.value,
        invite: invite,
      };
    }
    fetchData(json_data)
      .then((_resp) => {
        // {
        //   [NOTIFY_KIND_PARAM]: NotifyKind.OK,
        //   [NOTIFY_TEXT_PARAM]: `Successful ${props.config?.kind} please confirm token.`,
        // }
        navigate(props.config?.redirect);
      })
      .catch((e) => {
        // {
        //   [NOTIFY_KIND_PARAM]: NotifyKind.ERROR,
        //   [NOTIFY_TEXT_PARAM]: `Failed to ${props.config?.kind} please try again.`,
        // }
        navigate(props.pathname());
      });
    handleFormSubmitting(false);
  };

  const handleFormSubmitting = (submitting) => {
    setForm({ ...form(), submitting: submitting });
  };

  const request_config = (data) => {
    return {
      url: `${BENCHER_API_URL()}/v0/auth/${props.config?.kind}`,
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
        onClick={handleAuthFormSubmit}
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
