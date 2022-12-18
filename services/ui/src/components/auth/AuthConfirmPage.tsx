import axios from "axios";
import { useLocation, useNavigate, useSearchParams } from "solid-app-router";
import { createEffect, createMemo, createSignal } from "solid-js";
import AUTH_FIELDS from "./config/fields";
import Field from "../field/Field";
import {
  NotifyKind,
  notifyParams,
  pageTitle,
  post_options,
  validate_jwt,
} from "../site/util";
import Notification from "../site/Notification";
import FieldKind from "../field/kind";

const TOKEN_PARAM = "token";

const AuthConfirmPage = (props: {
  config: any;
  user: Function;
  handleUser: Function;
}) => {
  const navigate = useNavigate();
  const location = useLocation();
  const pathname = createMemo(() => location.pathname);
  const [searchParams, setSearchParams] = useSearchParams();

  if (searchParams[TOKEN_PARAM] && !validate_jwt(searchParams[TOKEN_PARAM])) {
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

  const post = async () => {
    const url = props.config?.form?.path;
    const no_token = null;
    const data = {
      token: token(),
    };
    const resp = await axios(post_options(url, no_token, data));
    return resp.data;
  };

  const handleFormSubmit = () => {
    handleFormSubmitting(true);
    post()
      .then((data) => {
        if (props.handleUser(data)) {
          navigate(
            notifyParams(props.config?.form?.redirect, NotifyKind.OK, "Ahoy!")
          );
        } else {
          navigate(
            notifyParams(
              pathname(),
              NotifyKind.ERROR,
              "Invalid user please try again."
            )
          );
        }
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

  createEffect(() => {
    if (validate_jwt(props.user()?.token)) {
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
    if (validate_jwt(jwt) && jwt !== submitted()) {
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
                <Field
                  kind={FieldKind.INPUT}
                  fieldKey="token"
                  label={true}
                  value={form()?.token?.value}
                  valid={form()?.token?.valid}
                  config={AUTH_FIELDS.token}
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
