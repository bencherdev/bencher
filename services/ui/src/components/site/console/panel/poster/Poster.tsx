import axios from "axios";
import {
  createSignal,
  createResource,
  createEffect,
  Suspense,
  For,
  Switch,
  Match,
} from "solid-js";
import SiteField from "../../../fields/SiteField";
import { Field } from "../../console";

const initForm = (fields) => {
  let newForm = {};
  fields.forEach((field) => {
    if (field.key) {
      newForm[field.key] = {};
      newForm[field.key].type = field.type;
      newForm[field.key].value = field.value;
      newForm[field.key].valid = field.valid;
      newForm[field.key].validate = field.validate;
    }
  });
  return newForm;
};

const Poster = (props) => {
  const [form, setForm] = createSignal(initForm(props.config?.fields));
  const [valid, setValid] = createSignal(false);

  const handleField = (key, value, valid) => {
    if (key && form()[key]) {
      setForm({
        ...form(),
        [key]: {
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
        <div class="box">
          <For each={props.config?.fields}>
            {(field, i) => (
              <SiteField
                key={i}
                kind={field.kind}
                fieldKey={field.key}
                label={field.label}
                value={form()[field.key]?.value}
                valid={form()[field.key]?.valid}
                config={field.config}
                handleField={handleField}
              />
            )}
          </For>
        </div>
      </div>
    </div>
  );
};

export default Poster;
