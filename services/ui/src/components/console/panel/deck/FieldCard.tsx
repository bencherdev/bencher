const FieldCard = (props) => {
  return (
    <div class="card">
      <div class="card-header">
        <div class="card-header-title">{props.card?.label}</div>
      </div>
      <div class="card-content">
        <div class="content">{props.value}</div>
      </div>
      {props.card?.is_allowed &&
        <div class="card-footer">
          <div class="card-footer-item">Update</div>
        </div>
      }
    </div>
  );
};

export default FieldCard;
