import Input from "./form/Input";
import Checkbox from "./form/Checkbox";
import Switch from "./form/Switch";
import Select from "./form/Select";
import { FieldKind } from "../console/config/types";

const SiteField = (props) => {
  function handleField(value) {
    switch (props.kind) {
      case FieldKind.CHECKBOX:
        props.handleField(props.fieldKey, value, value);
        break;
      case FieldKind.SWITCH:
        props.handleField(props.fieldKey, value, true);
        break;
      case FieldKind.SELECT:
        props.handleField(
          props.fieldKey,
          { ...props.value, selected: value },
          true
        );
        break;
      case FieldKind.INPUT:
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
      case FieldKind.CHECKBOX:
        return (
          <Checkbox
            value={props.value}
            config={props.config}
            handleField={handleField}
          />
        );
      case FieldKind.SWITCH:
        return (
          <Switch
            value={props.value}
            config={props.config}
            handleField={handleField}
          />
        );
      case FieldKind.SELECT:
        return (
          <Select
            value={props.value}
            config={props.config}
            handleField={handleField}
          />
        );
      case FieldKind.INPUT:
        return (
          <Input
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
      case FieldKind.CHECKBOX:
      case FieldKind.SWITCH:
      case FieldKind.SELECT:
      case FieldKind.TABLE:
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
