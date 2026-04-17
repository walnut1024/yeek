export interface FilterState {
  agent: string | null;
  project_path: string | null;
}

export const DEFAULT_FILTERS: FilterState = {
  agent: null,
  project_path: null,
};
