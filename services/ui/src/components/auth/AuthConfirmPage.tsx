import axios from "axios";
import { useSearchParams } from "solid-app-router";
import { createEffect, createMemo, createSignal } from "solid-js";
import { Field } from "../console/config/types";
import userFieldsConfig from "../fields/config/user/userFieldsConfig";
import SiteField from "../fields/SiteField";
import validator from "validator";

const TOKEN_PARAM = "token";

const AuthConfirmPage = (props: {
  config: any;
  handleTitle: Function;
  handleRedirect: Function;
  handleUser: Function;
  handleNotification: Function;
}) => {
  props.handleTitle(props.config?.title);

  const [searchParams, setSearchParams] = useSearchParams();

  if (
    !searchParams[TOKEN_PARAM] ||
    !validator.isJWT(searchParams[TOKEN_PARAM])
  ) {
    setSearchParams({ [TOKEN_PARAM]: null });
  }

  const token = createMemo(() => searchParams[TOKEN_PARAM]);

  const [submitted, setSubmitted] = createSignal();

  createEffect(() => {
    const jwt = token();
    if (jwt && validator.isJWT(jwt) && jwt !== submitted()) {
      setSubmitted(jwt);
      handleFormSubmit();
    }
  });

  const [form, setForm] = createSignal(initForm());

  createEffect(() => {
    const value = form()?.token?.value;
    if (value.length > 0) {
      setSearchParams({ [TOKEN_PARAM]: value });
    }

    const valid = form()?.token?.valid;
    if (valid !== form()?.valid) {
      setForm({ ...form(), valid: valid });
    }
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

  const handleFormSubmit = () => {
    handleFormSubmitting(true);
    const json_data = {
      token: token(),
    };
    fetchData(json_data)
      .then((resp) => {
        props.handleUser(resp.data);
        props.handleNotification({ status: "ok", text: "🐰 Ahoy!" });
        props.handleRedirect(props.config?.form?.redirect);
      })
      .catch((e) => {
        props.handleNotification({
          status: "error",
          text: `Failed to confirm token: ${e}`,
        });
      });
    handleFormSubmitting(false);
  };

  const handleFormSubmitting = (submitting) => {
    setForm({ ...form(), submitting: submitting });
  };

  const request_config = (data) => {
    return {
      url: props.config?.form?.path,
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
    <section class="section">
      <div class="container">
        <div class="columns is-centered">
          <div class="column is-two-fifths">
            <h2 class="title">{props.config?.title}</h2>
            <h3 class="subtitle">{props.config?.sub}</h3>

            <form class="box">
              <SiteField
                kind={Field.INPUT}
                fieldKey="token"
                label={true}
                value={form()?.token?.value}
                valid={form()?.token?.valid}
                config={userFieldsConfig.token}
                handleField={handleField}
              />

              <button
                class="button is-primary is-fullwidth"
                disabled={!form()?.valid || form()?.submitting}
                onClick={(e) => {
                  e.preventDefault();
                  handleFormSubmit();
                }}
              >
                Submit
              </button>
            </form>
          </div>
        </div>
      </div>
    </section>
  );
};

const initForm = () => {
  return {
    token: {
      value: "",
      valid: null,
    },
    valid: false,
    submitting: false,
  };
};

export default AuthConfirmPage;
