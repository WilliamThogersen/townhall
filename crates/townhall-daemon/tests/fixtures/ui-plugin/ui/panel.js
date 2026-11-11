const report = {};
try {
  report.storage = typeof window.localStorage;
} catch (e) {
  report.storage = e.name;
}
fetch("/api/info")
  .then(() => (report.api = "reached"))
  .catch(() => (report.api = "blocked"))
  .finally(() => {
    document.getElementById("out").textContent = JSON.stringify(report);
    parent.postMessage({ th: 1, type: "hello", apiVersion: 1, report }, "*");
  });
