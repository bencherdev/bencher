import axios from "axios";
import { useLocation, useNavigate, useSearchParams } from "solid-app-router";
import { createEffect, createMemo, createSignal, lazy } from "solid-js";
import { Field } from "../console/config/types";
import userFieldsConfig from "../fields/config/user/userFieldsConfig";
import SiteField from "../fields/SiteField";
import validator from "validator";
import { NotifyKind, notifyParams, pageTitle } from "../site/util";
import Notification from "../site/Notification";

const TOKEN_PARAM = "token";

const AuthConfirmPage = (props: {
  user: Function;
  config: any;
  handleUser: Function;
}) => {
  const navigate = useNavigate();
  const location = useLocation();
  const pathname = createMemo(() => location.pathname);
  const [searchParams, setSearchParams] = useSearchParams();

  if (
    searchParams[TOKEN_PARAM] &&
    !validator.isJWT(searchParams[TOKEN_PARAM].trim())
  ) {
    setSearchParams({ [TOKEN_PARAM]: null });
  }

  const token = createMemo(() =>
    searchParams[TOKEN_PARAM] ? searchParams[TOKEN_PARAM].trim() : null
  );
  const [submitted, setSubmitted] = createSignal();
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

  const handleFormSubmit = () => {
    handleFormSubmitting(true);
    const json_data = {
      token: token(),
    };
    fetchData(json_data)
      .then((resp) => {
        props.handleUser(resp.data);
        navigate(
          notifyParams(props.config?.form?.redirect, NotifyKind.OK, "Ahoy!")
        );
      })
      .catch((e) => {
        navigate(
          notifyParams(
            pathname(),
            NotifyKind.ERROR,
            "Failed to confirm token please try again."
          )
        );
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

  createEffect(() => {
    if (props.user().token && validator.isJWT(props.user().token)) {
      navigate("/console");
    }

    pageTitle(props.config?.title);

    const value = form()?.token?.value;
    if (value.length > 0) {
      setSearchParams({ [TOKEN_PARAM]: value });
    }

    const valid = form()?.token?.valid;
    if (valid !== form()?.valid) {
      setForm({ ...form(), valid: valid });
    }

    const jwt = token();
    if (jwt && validator.isJWT(jwt) && jwt !== submitted()) {
      setSubmitted(jwt);
      handleFormSubmit();
    }
  });

  return (
    <>
      <Notification />

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
    </>
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
