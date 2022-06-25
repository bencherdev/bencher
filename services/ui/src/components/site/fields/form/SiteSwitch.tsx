const SiteSwitch = (props) => {
  return (
    <div class="field" id={props.config.label}>
      <input
        id={props.config.label}
        type="checkbox"
        name={props.config.label}
        class="switch"
        checked={props.value}
        onChange={(e) => props.handleField(e)}
      />
      <label for={props.config.label}></label>
    </div>
  );
};

export default SiteSwitch;
