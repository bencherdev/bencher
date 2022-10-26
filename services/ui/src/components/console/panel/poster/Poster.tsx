import axios from "axios";
import { createResource, createSignal, For, Match, Switch } from "solid-js";
import SiteField from "../../../fields/SiteField";
import validator from "validator";
import { getToken } from "../../../site/util";
import { Field } from "../../config/types";

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

const options = (url: string, token: string, data: any) => {
  return {
    url: url,
    method: "POST",
    data: data,
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
  };
};

const Poster = (props) => {
  const [form, setForm] = createSignal(initForm(props.config?.fields));
  const [valid, setValid] = createSignal(false);

  const postData = async (data) => {
    try {
      const token = getToken();
      if (token && !validator.isJWT(token)) {
        return;
      }

      await axios(
        options(props.config?.url?.(props.path_params()), token, data)
      );
      props.handleRedirect(props.config?.path?.(props.pathname()));
    } catch (error) {
      console.error(error);
    }
  };

  function sendForm(e) {
    e.preventDefault();
    handleFormSubmitting(true);
    let data = {};
    for (let key of Object.keys(form())) {
      switch (form()?.[key]?.kind) {
        case Field.SELECT:
          data[key] = form()?.[key]?.value?.selected;
          break;
        default:
          if (!form()?.[key]?.value && form()?.[key]?.nullify) {
            data[key] = null;
          } else {
            data[key] = form()?.[key]?.value;
          }
      }
    }
    postData(data);
    handleFormSubmitting(false);
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
      setValid(getValid());
    }
  };

  function getValid() {
    let allValid = true;
    Object.values(form()).forEach((field) => {
      if (field.validate && !field.valid) {
        allValid = false;
      }
    });
    return allValid;
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
            disabled={!valid() || form()?.submitting}
            onClick={sendForm}
          >
            Submit
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
      <Match when={props.field?.kind === Field.HIDDEN}></Match>
    </Switch>
  );
};

export default Poster;
