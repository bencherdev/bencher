import FieldHelp from "./FieldHelp";
import SiteInput from "./form/SiteInput";
import SiteTextarea from "./form/SiteTextarea";
import SiteCheckbox from "./form/SiteCheckbox";
import SiteSwitch from "./form/SiteSwitch";
import SiteSelect from "./form/SiteSelect";
import { Field } from "../console/config/types";

const SiteField = (props) => {
  function handleField(event, field = null) {
    console.log(event);

    switch (props.kind) {
      case Field.CHECKBOX:
        props.handleField(
          props.fieldKey,
          event.target.checked,
          event.target.checked
        );
        break;
      case Field.SWITCH:
        props.handleField(props.fieldKey, event, true);
        break;
      case Field.SELECT:
        props.handleField(
          props.fieldKey,
          { ...props.value, selected: event.target.value },
          true
        );
        break;
      default:
        props.handleField(
          props.fieldKey,
          event.target.value,
          props.config.validate
            ? props.config.validate(event.target.value)
            : true
        );
    }
  }

  function getField() {
    switch (props.kind) {
      case Field.TEXTAREA:
        return (
          <SiteTextarea
            value={props.value}
            config={props.config}
            handleField={handleField}
          />
        );
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
      default:
        return (
          <SiteInput
            value={props.value}
            valid={props.valid}
            config={props.config}
            handleField={handleField}
          />
        );
    }
  }

  function shouldValidate() {
    switch (props.kind) {
      case Field.CHECKBOX:
      case Field.SWITCH:
      case Field.SELECT:
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
        <FieldHelp fieldText={props.config.help} fieldValid={props.valid} />
      )}
    </div>
  );
};

export default SiteField;
