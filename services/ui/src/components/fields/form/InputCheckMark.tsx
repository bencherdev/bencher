function InputCheckMark({ fieldValid }) {
  return (
    <div>
      {fieldValid && (
        <span class="icon is-small is-right">
          <i class="fas fa-check"></i>
        </span>
      )}
    </div>
  );
}

export default InputCheckMark;
