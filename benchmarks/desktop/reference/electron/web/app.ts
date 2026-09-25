import initialIssues from "./issues.generated.json";
import { workload } from "../workload";

const issues = initialIssues;
let query = "";
let filter = "All issues";
let page = 0;
let selected = 0;
const get = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const list = get("issues");
const rows = new Map<number, HTMLButtonElement>();
const filterButtons = [...document.querySelectorAll<HTMLButtonElement>("[data-filter]")];

function matching() {
  const needle = query.trim().toLowerCase();
  return issues.flatMap((issue, index) =>
    (filter === "All issues" ||
      (filter === "Open" ? issue.status !== "Done" : issue.status === "Done")) &&
    `${issue.id} ${issue.title} ${issue.project} ${issue.owner}`.toLowerCase().includes(needle)
      ? [index]
      : [],
  );
}

function render() {
  const matches = matching();
  page = Math.min(page, Math.max(0, Math.ceil(matches.length / workload.pageSize) - 1));
  const visible = matches.slice(page * workload.pageSize, (page + 1) * workload.pageSize);
  const keep = new Set(visible);
  for (const [index, row] of rows)
    if (!keep.has(index)) {
      row.remove();
      rows.delete(index);
    }
  for (const index of visible) {
    const issue = issues[index]!;
    let row = rows.get(index);
    if (!row) {
      row = document.createElement("button");
      row.className = "issue-row";
      row.dataset.index = String(index);
      const title = document.createElement("span");
      title.className = "row-title";
      const meta = document.createElement("span");
      meta.className = "row-meta";
      row.append(title, meta);
      row.onclick = () => {
        selected = index;
        render();
      };
      rows.set(index, row);
    }
    row.setAttribute("aria-pressed", String(index === selected));
    row.children[0]!.textContent = issue.title;
    row.children[1]!.textContent = `${issue.id}  ·  ${issue.project}  ·  ${issue.status}  ·  ${issue.owner}`;
    list.append(row);
  }
  get("empty").hidden = visible.length !== 0;
  get("results").textContent = `${matches.length} issues`;
  get("page-info").textContent =
    `Page ${page + 1} of ${Math.max(1, Math.ceil(matches.length / workload.pageSize))}`;
  get<HTMLButtonElement>("previous").disabled = page === 0;
  get<HTMLButtonElement>("next").disabled = (page + 1) * workload.pageSize >= matches.length;
  const completed = issues.filter((issue) => issue.status === "Done").length;
  get("workspace-summary").textContent =
    `${issues.length - completed} open · ${completed} completed`;
  filterButtons.forEach((button) =>
    button.setAttribute("aria-pressed", String(button.dataset.filter === filter)),
  );
  const issue = issues[selected]!;
  get("detail-id").textContent = `${issue.id} / ${issue.project}`;
  get("detail-title").textContent = issue.title;
  get("detail-status").textContent = issue.status;
  get("detail-owner").textContent = issue.owner;
  get("detail-priority").textContent = issue.priority;
  get("description").textContent = issue.description;
  get<HTMLTextAreaElement>("notes").value = issue.notes;
  get("complete").textContent = issue.status === "Done" ? "Reopen issue" : "Mark complete";
}
get<HTMLInputElement>("search").oninput = (event) => {
  query = (event.target as HTMLInputElement).value;
  page = 0;
  list.scrollTop = 0;
  render();
};
filterButtons.forEach((button) => {
  button.onclick = () => {
    filter = button.dataset.filter!;
    page = 0;
    list.scrollTop = 0;
    render();
  };
});
get("previous").onclick = () => {
  page--;
  list.scrollTop = 0;
  render();
};
get("next").onclick = () => {
  page++;
  list.scrollTop = 0;
  render();
};
get("complete").onclick = () => {
  const issue = issues[selected]!;
  issue.status = issue.status === "Done" ? "Open" : "Done";
  render();
};
get<HTMLTextAreaElement>("notes").oninput = (event) => {
  issues[selected]!.notes = (event.target as HTMLTextAreaElement).value;
};
render();

// Check actual content and layout before allowing memory sampling.
let attempts = 0;
function signalReady() {
  requestAnimationFrame(() =>
    requestAnimationFrame(async () => {
      if (++attempts > 20) throw new Error("Could not size the issue tracker viewport");
      if (
        issues.length !== workload.records ||
        rows.size !== workload.pageSize ||
        get("detail-title").textContent !== issues[0]!.title ||
        [list, get("notes"), get("search")].some(
          (node) => node.getBoundingClientRect().height === 0,
        )
      ) {
        throw new Error("Issue tracker did not render its workload");
      }
      const tauri = (
        window as Window & {
          __TAURI__?: { core: { invoke: (command: string, args: object) => Promise<boolean> } };
        }
      ).__TAURI__;
      if (tauri) {
        if (
          !(await tauri.core.invoke("workload_ready", { width: innerWidth, height: innerHeight }))
        )
          signalReady();
      } else {
        document.title =
          innerWidth === workload.width && innerHeight === workload.height
            ? workload.readyTitle
            : `Invalid issue tracker viewport: ${innerWidth} × ${innerHeight}`;
      }
    }),
  );
}
signalReady();
