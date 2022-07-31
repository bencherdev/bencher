function FieldHelp({ fieldText, fieldValid }) {
  return (
    <p
      class={(() => {
        switch (fieldValid) {
          case true:
            return "help".concat(" is-success");
          case false:
            return "help".concat(" is-danger");
          default:
            return "help";
        }
      })()}
    >
      {fieldText}
    </p>
  );
}

export default FieldHelp;
