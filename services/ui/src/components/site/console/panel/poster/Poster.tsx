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
              <Switch fallback={<div>Unknown Field: {field}</div>}>
                <Match when={field.kind === Field.INPUT}>
                  <div>TODO</div>
                  {/* <SiteField
                    type="input"
                    fieldKey="username"
                    label={true}
                    value={form()?.username?.value}
                    valid={form()?.username?.valid}
                    config={field.config}
                    handleField={handleField}
                  /> */}
                </Match>
              </Switch>
            )}
          </For>
        </div>
      </div>
    </div>
  );
};

export default Poster;
