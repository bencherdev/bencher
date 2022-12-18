import { createResource, createSignal, Match, Switch } from "solid-js";
import SiteField from "../../../fields/SiteField";
import { patch_options, validate_jwt } from "../../../site/util";
import { Display } from "../../config/types";
import axios from "axios";
import FieldKind from "../../../fields/kind";

const FieldCard = (props) => {
  const [update, setUpdate] = createSignal(false);

  const toggleUpdate = () => {
    setUpdate(!update());
  };

  return (
    <Switch
      fallback={
        <ViewCard
          card={props.card}
          value={props.value}
          path_params={props.path_params}
          toggleUpdate={toggleUpdate}
        />
      }
    >
      <Match when={update()}>
        <UpdateCard
          user={props.user}
          card={props.card}
          value={props.value}
          path_params={props.path_params}
          url={props.url}
          toggleUpdate={toggleUpdate}
          handleRefresh={props.handleRefresh}
        />
      </Match>
    </Switch>
  );
};

const ViewCard = (props) => {
  const [is_allowed] = createResource(props.path_params, (path_params) =>
    props.card?.is_allowed?.(path_params)
  );

  return (
    <div class="card">
      <div class="card-header">
        <div class="card-header-title">{props.card?.label}</div>
      </div>
      <div class="card-content">
        <div class="content">
          <Switch fallback={props.value}>
            <Match when={props.card?.display === Display.SWITCH}>
              <div class="field">
                <input
                  type="checkbox"
                  class="switch"
                  checked={props.value}
                  disabled={true}
                />
                <label></label>
              </div>
            </Match>
            <Match when={props.card?.display === Display.SELECT}>
              {props.card?.field?.value?.options.reduce((field, option) => {
                if (props.value === option.value) {
                  return option.option;
                } else {
                  return field;
                }
              }, props.value)}
            </Match>
          </Switch>
        </div>
      </div>
      {is_allowed() && (
        <div class="card-footer">
          <a
            class="card-footer-item"
            onClick={(e) => {
              e.preventDefault();
              props.toggleUpdate();
            }}
          >
            Update
          </a>
        </div>
      )}
    </div>
  );
};

const initForm = (field, value) => {
  switch (field?.kind) {
    case FieldKind.SELECT:
      field.value.selected = value;
      break;
    default:
      field.value = value;
  }

  return {
    [field?.key]: {
      kind: field?.kind,
      label: field?.label,
      value: field?.value,
      valid: field?.valid,
      validate: field?.validate,
      nullify: field?.nullify,
    },
    submitting: false,
  };
};

const UpdateCard = (props) => {
  const [form, setForm] = createSignal(
    initForm(props.card?.field, props.value)
  );
  const [valid, setValid] = createSignal(false);

  const is_sendable = (): boolean => {
    return !form()?.submitting && valid() && !is_value_unchanged();
  };

  const patch = async (data) => {
    try {
      const token = props.user()?.token;
      if (!validate_jwt(token)) {
        return;
      }

      const url = props.url();
      await axios(patch_options(url, token, data));
      props.handleRefresh();
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

    patch(data).then(() => handleFormSubmitting(false));
  }

  const is_value_unchanged = () => {
    switch (props.card?.field?.kind) {
      case FieldKind.SELECT:
        return (
          props.value === form()?.[props.card?.field?.key]?.value?.selected
        );
      default:
        return props.value === form()?.[props.card?.field?.key]?.value;
    }
  };

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
    <div class="card">
      <div class="card-header">
        <div class="card-header-title">{props.card?.label}</div>
      </div>
      <div class="card-content">
        <div class="content">
          <SiteField
            kind={props.card?.field?.kind}
            fieldKey={props.card?.field?.key}
            value={form()?.[props.card?.field?.key]?.value}
            valid={form()?.[props.card?.field?.key]?.valid}
            config={props.card?.field?.config}
            handleField={handleField}
          />
        </div>
      </div>
      <div class="card-footer">
        <a
          class={`card-footer-item ${is_sendable ? "" : "disabled"}`}
          onClick={sendForm}
        >
          Save
        </a>
        <a
          class="card-footer-item"
          onClick={(e) => {
            e.preventDefault();
            props.toggleUpdate();
          }}
        >
          Cancel
        </a>
      </div>
    </div>
  );
};

export default FieldCard;
