const Input = (props) => {
  return (
    <div class="control has-icons-left has-icons-right">
      <span class="icon is-small is-left">
        <i class={props.config.icon}></i>
      </span>
      <input
        class="input"
        type={props.config.type}
        placeholder={props.config.placeholder}
        value={props.value}
        disabled={props.config.disabled}
        onInput={(event) => props.handleField(event.target?.value)}
      />
      {props.valid && (
        <span class="icon is-small is-right">
          <i class="fas fa-check"></i>
        </span>
      )}
    </div>
  );
};

export default Input;
