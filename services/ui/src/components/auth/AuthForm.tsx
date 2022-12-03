import { createSignal, createEffect, Accessor, createMemo } from "solid-js";
import axios from "axios";

import SiteField from "../fields/SiteField";
import AUTH_FIELDS from "./config/fields";
import { Field } from "../console/config/types";
import { FormKind } from "./config/types";
import {
  BENCHER_API_URL,
  NotifyKind,
  notifyParams,
  post_options,
  validate_jwt,
} from "../site/util";
import { useLocation, useNavigate } from "solid-app-router";

export interface Props {
  config: any;
  invite: Function;
  user: Function;
  handleUser: Function;
}

export const AuthForm = (props: Props) => {
  const navigate = useNavigate();
  const location = useLocation();
  const pathname = createMemo(() => location.pathname);

  const [form, setForm] = createSignal(initForm());

  const handleField = (key, value, valid) => {
    setForm({
      ...form(),
      [key]: {
        value: value,
        valid: valid,
      },
    });
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

  const handleFormValid = () => {
    var valid = validateForm();
    if (valid !== form()?.valid) {
      setForm({ ...form(), valid: valid });
    }
  };

  const handleFormSubmitting = (submitting) => {
    setForm({ ...form(), submitting: submitting });
  };

  const post = async (data: {
    name: null | string;
    slug: null | string;
    email: string;
    invite: null | string;
  }) => {
    const url = `${BENCHER_API_URL()}/v0/auth/${props.config?.kind}`;
    const no_token = null;
    let resp = await axios(post_options(url, no_token, data));
    return resp.data;
  };

  const handleAuthFormSubmit = (event) => {
    event.preventDefault();
    handleFormSubmitting(true);
    const invite_token = props.invite();
    let invite: string | null;
    if (validate_jwt(invite_token)) {
      invite = invite_token;
    } else {
      invite = null;
    }

    let data;
    if (props.config?.kind === FormKind.SIGNUP) {
      const signup_form = form();
      data = {
        name: signup_form.username.value?.trim(),
        slug: null,
        email: signup_form.email.value?.trim(),
        invite: invite,
      };
    } else if (props.config?.kind === FormKind.LOGIN) {
      const login_form = form();
      data = {
        email: login_form.email.value?.trim(),
        invite: invite,
      };
    }

    post(data)
      .then((_resp) => {
        navigate(
          notifyParams(
            props.config?.redirect,
            NotifyKind.OK,
            `Successful ${props.config?.kind} please confirm token.`
          )
        );
      })
      .catch((_e) => {
        navigate(
          notifyParams(
            pathname(),
            NotifyKind.ERROR,
            `Failed to ${props.config?.kind} please try again.`
          )
        );
      });

    handleFormSubmitting(false);
  };

  createEffect(() => {
    handleFormValid();
  });

  return (
    <form class="box">
      {props.config?.kind === FormKind.SIGNUP && (
        <SiteField
          kind={Field.INPUT}
          fieldKey="username"
          label={true}
          value={form()?.username?.value}
          valid={form()?.username?.valid}
          config={AUTH_FIELDS.username}
          handleField={handleField}
        />
      )}

      <SiteField
        kind={Field.INPUT}
        fieldKey="email"
        label={true}
        value={form()?.email?.value}
        valid={form()?.email?.valid}
        config={AUTH_FIELDS.email}
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
            config={AUTH_FIELDS.consent}
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
