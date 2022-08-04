import axios from "axios";
import { createSignal, For } from "solid-js";
import SiteField from "../../../fields/SiteField";

const initForm = (fields) => {
  let newForm = {};
  fields.forEach((field) => {
    if (field.key) {
      newForm[field.key] = {};
      newForm[field.key].type = field.type;
      newForm[field.key].value = field.value;
      newForm[field.key].valid = field.valid;
      newForm[field.key].validate = field.validate;
      newForm[field.key].nullify = field.nullify;
    }
  });
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

  const postData = async (url, data) => {
    try {
      const token = JSON.parse(window.localStorage.getItem("user"))?.uuid;
      if (typeof token !== "string") {
        return;
      }
      let resp = await axios(options(url, token, data));
      const resp_data = resp.data;
      console.log(resp_data);
      props.handleRedirect(props.config?.path(props.pathname()));
      return resp_data;
    } catch (error) {
      console.error(error);
    }
  };

  function sendForm(url, form) {
    let data = {};
    for (let key of Object.keys(form)) {
      switch (form[key].type) {
        case "select":
          data[key] = form[key].value.selected;
          break;
        default:
          console.log(form[key]);
          if (!form[key].value && form[key].nullify) {
            data[key] = null;
          } else {
            data[key] = form[key].value;
          }
      }
    }
    console.log(data);
    postData(url, data);
  }

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
          <br />
          <button
            class="button is-primary is-fullwidth"
            disabled={!valid()}
            onClick={(e) => {
              e.preventDefault();
              sendForm(props.config?.url, form());
            }}
          >
            Submit
          </button>
        </div>
      </div>
    </div>
  );
};

export default Poster;
