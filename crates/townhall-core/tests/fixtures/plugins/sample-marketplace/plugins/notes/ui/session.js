const status = document.getElementById("status");

async function count() {
  const tasks = await townhall.api.call("GET", "/api/plugin/v1/tasks");
  status.textContent = `${tasks.length} tasks in this townhall`;
}

townhall.ready
  .then(() => townhall.ui.resize(64))
  .then(count)
  .then(() => townhall.events.subscribe(["taskUpdated"], count))
  .catch((e) => {
    status.textContent = e.message;
    townhall.ui.toast(e.message, "error");
  });
