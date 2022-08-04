import { Match, Switch } from "solid-js";
import { Card } from "../../config/types";
import FieldCard from "./FieldCard";

const DeckCard = (props: { kind: Card; field: string; value: string }) => {
  return (
    <Switch
      fallback={
        <div>
          {props.field}: {props.value}
        </div>
      }
    >
      <Match when={props.kind === Card.FIELD}>
        <FieldCard field={props.field} value={props.value} />
      </Match>
      <Match when={props.kind === Card.TABLE}>
        <div>Table Card</div>
      </Match>
    </Switch>
  );
};

export default DeckCard;
