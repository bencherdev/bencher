import { createResource } from "solid-js";

const FieldCard = (props) => {
  const [is_allowed] = createResource(props.path_params, (path_params) => props.card?.is_allowed?.(path_params));

  return (
    <div class="card">
      <div class="card-header">
        <div class="card-header-title">{props.card?.label}</div>
      </div>
      <div class="card-content">
        <div class="content">{props.value}</div>
      </div>
      {is_allowed() &&
        <div class="card-footer">
          <div class="card-footer-item">Update</div>
        </div>
      }
    </div>
  );
};

export default FieldCard;
