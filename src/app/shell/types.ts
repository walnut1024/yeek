export type SortMode = "updated_at" | "started_at" | "title";

export interface FilterState {
  agent: string | null;
  project_path: string | null;
  visibility: string;
  pinned_only: boolean;
}

export const DEFAULT_FILTERS: FilterState = {
  agent: null,
  project_path: null,
  visibility: "visible",
  pinned_only: false,
};
