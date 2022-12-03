const SiteSwitch = (props) => {
  return (
    <div class="field" id={props.config?.label}>
      <input
        id={props.config?.label}
        type="checkbox"
        class="switch"
        name={props.config?.label}
        checked={props.value}
        disabled={props.config?.disabled}
        onInput={(e) => props.handleField(e)}
      />
      <label for={props.config?.label}></label>
    </div>
  );
};

export default SiteSwitch;
