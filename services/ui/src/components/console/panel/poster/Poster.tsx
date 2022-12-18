import axios from "axios";
import {
  createMemo,
  createResource,
  createSignal,
  For,
  Match,
  Switch,
} from "solid-js";
import SiteField from "../../../fields/SiteField";
import { post_options, validate_jwt } from "../../../site/util";
import { useLocation, useNavigate } from "solid-app-router";
import FieldKind from "../../../fields/kind";

const initForm = (fields) => {
  let newForm = {};
  fields.forEach((field) => {
    if (field.key) {
      newForm[field.key] = {};
      newForm[field.key].kind = field.kind;
      newForm[field.key].label = field.label;
      newForm[field.key].value = field.value;
      newForm[field.key].valid = field.valid;
      newForm[field.key].validate = field.validate;
      newForm[field.key].nullify = field.nullify;
    }
  });
  newForm.submitting = false;
  return newForm;
};

const Poster = (props) => {
  const navigate = useNavigate();
  const location = useLocation();
  const pathname = createMemo(() => location.pathname);

  const [form, setForm] = createSignal(initForm(props.config?.fields));
  const [valid, setValid] = createSignal(false);

  // setInterval(() => console.log(form()), 3000);

  const is_sendable = (): boolean => {
    return !form()?.submitting && valid();
  };

  const post = async (data: {}) => {
    try {
      const token = props.user()?.token;
      if (!validate_jwt(props.user()?.token)) {
        return;
      }

      const url = props.config?.url?.(props.path_params());
      await axios(post_options(url, token, data));
      navigate(props.config?.path?.(pathname()));
    } catch (error) {
      console.error(error);
    }
  };

  function sendForm(e) {
    e.preventDefault();
    if (!is_sendable()) {
      return;
    }

    handleFormSubmitting(true);
    let data = {};
    for (let key of Object.keys(form())) {
      switch (form()?.[key]?.kind) {
        case FieldKind.SELECT:
          data[key] = form()?.[key]?.value?.selected;
          break;
        default:
          if (!form()?.[key]?.value && form()?.[key]?.nullify) {
            data[key] = null;
          } else {
            const value = form()?.[key]?.value;
            if (typeof value === "string") {
              data[key] = value.trim();
            } else {
              data[key] = value;
            }
          }
      }
    }

    post(data).then(() => handleFormSubmitting(false));
  }

  const handleFormSubmitting = (submitting) => {
    setForm({ ...form(), submitting: submitting });
  };

  const handleField = (key, value, valid) => {
    if (key && form()?.[key]) {
      setForm({
        ...form(),
        [key]: {
          ...form()?.[key],
          value: value,
          valid: valid,
        },
      });
      setValid(isValid());
    }
  };

  function isValid() {
    const form_values = Object.values(form());
    for (let i = 0; i < form_values.length; i++) {
      if (form_values[i]?.validate && !form_values[i]?.valid) {
        return false;
      }
    }
    return true;
  }

  return (
    <div class="columns">
      <div class="column">
        <form class="box">
          <For each={props.config?.fields}>
            {(field, i) => (
              <PosterField
                field={field}
                i={i}
                form={form}
                handleField={handleField}
                path_params={props.path_params}
              />
            )}
          </For>
          <br />
          <button
            class="button is-primary is-fullwidth"
            disabled={!is_sendable()}
            onClick={sendForm}
          >
            Save
          </button>
        </form>
      </div>
    </div>
  );
};

const PosterField = (props) => {
  const [_hidden_field] = createResource(props.path_params, (path_params) => {
    const path_param = props.field.path_param;
    if (path_param) {
      props.handleField(props.field.key, path_params?.[path_param], true);
      return path_params?.[path_param];
    }
  });

  return (
    <Switch
      fallback={
        <SiteField
          key={props.i}
          kind={props.field?.kind}
          label={props.form()?.[props.field?.key]?.label}
          fieldKey={props.field?.key}
          value={props.form()?.[props.field?.key]?.value}
          valid={props.form()?.[props.field?.key]?.valid}
          config={props.field?.config}
          handleField={props.handleField}
        />
      }
    >
      <Match when={props.field?.kind === FieldKind.HIDDEN}></Match>
    </Switch>
  );
};

export default Poster;
