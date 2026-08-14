export type DesktopMenuAction =
  | "project.new"
  | "project.open"
  | "project.close"
  | "app.quit"
  | "edit.world"
  | "edit.propose"
  | "view.palette"
  | "view.changes"
  | "view.home"
  | "view.world"
  | "view.chronology"
  | "view.assistant"
  | "view.narrative"
  | "view.simulation"
  | "view.imports"
  | "view.versions"
  | "settings.open"
  | "help.open"
  | "help.onboarding"
  | "help.about";

export type DesktopActionRequest = {
  id: number;
  action: DesktopMenuAction;
};
