import SiteInput from "./form/SiteInput";
import SiteCheckbox from "./form/SiteCheckbox";
import SiteSwitch from "./form/SiteSwitch";
import SiteSelect from "./form/SiteSelect";
import { Field } from "../console/config/types";

const SiteField = (props) => {
  function handleField(value) {
    switch (props.kind) {
      case Field.CHECKBOX:
        props.handleField(props.fieldKey, value, value);
        break;
      case Field.SWITCH:
        props.handleField(props.fieldKey, value, true);
        break;
      case Field.SELECT:
        props.handleField(
          props.fieldKey,
          { ...props.value, selected: value },
          true
        );
        break;
      case Field.INPUT:
        props.handleField(
          props.fieldKey,
          value,
          props.config.validate ? props.config.validate(value) : true
        );
        break;
    }
  }

  function getField() {
    switch (props.kind) {
      case Field.CHECKBOX:
        return (
          <SiteCheckbox
            value={props.value}
            config={props.config}
            handleField={handleField}
          />
        );
      case Field.SWITCH:
        return (
          <SiteSwitch
            value={props.value}
            config={props.config}
            handleField={handleField}
          />
        );
      case Field.SELECT:
        return (
          <SiteSelect
            value={props.value}
            config={props.config}
            handleField={handleField}
          />
        );
      case Field.INPUT:
        return (
          <SiteInput
            value={props.value}
            valid={props.valid}
            config={props.config}
            handleField={handleField}
          />
        );
      default:
        return <div>UNKNOWN FIELD</div>;
    }
  }

  function shouldValidate() {
    switch (props.kind) {
      case Field.CHECKBOX:
      case Field.SWITCH:
      case Field.SELECT:
      case Field.TABLE:
        return false;
      default:
        return true;
    }
  }

  return (
    <div class="field">
      {props.label && <label class="label is-medium">{props.label}</label>}
      {getField()}
      {shouldValidate() && props.valid === false && (
        <p class="help is-danger">{props.config.help}</p>
      )}
    </div>
  );
};

export default SiteField;
