import {
  badge,
  block,
  button,
  humanize,
  labelForUri,
  objectKindFromUri,
  pathForUri,
  shortId,
} from "./helpers.js";
import {
  kindFilters,
  kinds,
  recentsEmpty,
  recentsList,
  resultsEmpty,
  resultsList,
  resultSummary,
  state,
  treeEmpty,
  treeRoot,
} from "./state.js";
import type { LogicalVfsNode } from "./types.js";
import { refreshNavigation, selectUri } from "./workspace.js";

function renderKindFilters(): void {
  kindFilters.replaceChildren(
    ...kinds.map((kind) => {
      const item = button(kind.label, "kind-filter secondary");
      item.setAttribute("aria-pressed", String(state.activeKind === kind.value));
      item.addEventListener("click", () => {
        state.activeKind = kind.value;
        void refreshNavigation();
      });
      return item;
    }),
  );
}

export {
  renderKindFilters,
  renderRecents,
  renderResults,
  renderTree,
};

function renderRecents(): void {
  const items = state.recentUris.slice(0, 8);
  recentsEmpty.hidden = items.length > 0;
  recentsList.replaceChildren(
    ...items.map((uri) => {
      const item = button(labelForUri(uri), "linked-button");
      item.setAttribute("aria-current", String(uri === state.selectedUri));
      const snippet = document.createElement("p");
      snippet.className = "linked-button-snippet";
      snippet.textContent = pathForUri(state.logicalTree, uri) ?? uri;
      const meta = block("badge-row");
      const kind = objectKindFromUri(uri);
      if (kind) {
        meta.append(badge(humanize(kind), "kind"));
      }
      item.append(meta, snippet);
      item.addEventListener("click", () => {
        void selectUri(uri);
      });
      return item;
    }),
  );
}

function renderTree(): void {
  const tree = state.logicalTree;
  if (!tree || tree.children.length === 0) {
    treeRoot.replaceChildren();
    treeEmpty.hidden = false;
    return;
  }
  treeEmpty.hidden = true;
  treeRoot.replaceChildren(renderTreeNodes(tree.children));
}

function renderTreeNodes(nodes: LogicalVfsNode[]): HTMLOListElement {
  const list = document.createElement("ol");
  list.className = "tree-list";
  for (const node of nodes) {
    const item = document.createElement("li");
    if (node.type === "object") {
      const objectButton = button(node.name, "tree-button");
      objectButton.setAttribute("aria-current", String(node.uri === state.selectedUri));
      objectButton.addEventListener("click", () => {
        void selectUri(node.uri);
      });
      const meta = document.createElement("div");
      meta.className = "tree-path";
      meta.textContent = node.uri;
      item.append(objectButton, meta);
    } else {
      const label = document.createElement("div");
      label.className = "tree-label";
      label.textContent = node.name;
      const branch = document.createElement("div");
      branch.className = "tree-branch";
      branch.append(renderTreeNodes(node.children));
      item.append(label, branch);
    }
    list.append(item);
  }
  return list;
}

function renderResults(): void {
  const summaryParts = [`${state.searchHits.length} resultado${state.searchHits.length === 1 ? "" : "s"}`];
  if (state.activeKind !== "all") {
    summaryParts.push(humanize(state.activeKind).toLowerCase());
  }
  if (state.queryText.trim()) {
    summaryParts.push(`para “${state.queryText.trim()}”`);
  }
  resultSummary.textContent = summaryParts.join(" · ");

  resultsEmpty.hidden = state.searchHits.length > 0;
  resultsList.replaceChildren(
    ...state.searchHits.map((result) => {
      const item = button("", "result-item");
      item.setAttribute("role", "option");
      item.setAttribute("aria-current", String(result.uri === state.selectedUri));
      item.setAttribute("aria-selected", String(result.uri === state.selectedUri));
      const title = document.createElement("div");
      title.className = "result-item-title";
      title.textContent = result.snippet;
      const meta = block("badge-row");
      meta.append(
        badge(humanize(result.object_type), "kind"),
        badge(humanize(result.classification), result.classification === "no_evidence" ? "warning" : "info"),
        badge(humanize(result.authority), "context"),
      );
      const hint = document.createElement("p");
      hint.className = "result-item-snippet";
      hint.textContent = `${shortId(result.object_id)} · ${result.provenance}`;
      item.append(title, meta, hint);
      item.addEventListener("click", () => {
        void selectUri(result.uri);
      });
      return item;
    }),
  );
}
