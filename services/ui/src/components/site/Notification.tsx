import { useSearchParams } from "solid-app-router";
import { createMemo, createSignal, Match, Switch } from "solid-js";
import {
  NotifyKind,
  NOTIFY_KIND_PARAM,
  NOTIFY_TEXT_PARAM,
  isNotifyKind,
  isNotifyText,
} from "./util";

const Notification = (props) => {
  const [searchParams, setSearchParams] = useSearchParams();

  // if (!isNotifyKind(searchParams[NOTIFY_KIND_PARAM])) {
  //   setSearchParams({ [NOTIFY_KIND_PARAM]: null });
  // }

  // if (!isNotifyText(searchParams[NOTIFY_TEXT_PARAM])) {
  //   setSearchParams({ [NOTIFY_TEXT_PARAM]: null });
  // }

  const notify_kind = createMemo(() =>
    parseInt(searchParams[NOTIFY_KIND_PARAM])
  );

  const notify_text = createMemo(() => searchParams[NOTIFY_TEXT_PARAM]);

  const removeNotification = () => {
    setSearchParams({
      [NOTIFY_KIND_PARAM]: null,
      [NOTIFY_TEXT_PARAM]: null,
    });
  };

  const getNotification = () => {
    let color: string;
    switch (notify_kind()) {
      case NotifyKind.OK:
        color = "is-success";
        break;
      case NotifyKind.ALERT:
        color = "is-primary";
        break;
      case NotifyKind.ERROR:
        color = "is-danger";
        break;
      default:
        color = "";
    }
    setTimeout(() => {
      removeNotification();
    }, 4000);
    return (
      <div class={`notification ${color}`}>
        🐰 {notify_text()}
        <button
          class="delete"
          onClick={(e) => {
            e.preventDefault();
            removeNotification();
          }}
        />
      </div>
    );
  };

  return (
    <div>
      <Switch fallback={<p>None</p>}>
        <Match
          when={
            // isNotifyKind(searchParams[NOTIFY_KIND_PARAM]) &&
            isNotifyText(notify_text())
          }
        >
          <section class="section">
            <div class="container">{getNotification()}</div>
          </section>
        </Match>
      </Switch>
    </div>
  );
};

export default Notification;
