import { describe, expect, it } from "vitest";

import { commandToRoute } from "../transport";

describe("HTTP transport route mapping", () => {
  it("unwraps Tauri request args into GET query parameters", () => {
    const route = commandToRoute("browse_sessions", {
      request: {
        sort: "updated_at",
        limit: 100,
        agent: "claude_code",
      },
    });

    expect(route).toEqual({
      method: "GET",
      path: "/api/sessions?sort=updated_at&limit=100&agent=claude_code",
      body: undefined,
    });
  });

  it("maps search query to the HTTP q parameter and preserves agent filter", () => {
    const route = commandToRoute("search_sessions", {
      request: {
        query: "WorkBenc",
        limit: 100,
        agent: "codex",
      },
    });

    expect(route).toEqual({
      method: "GET",
      path: "/api/sessions/search?q=WorkBenc&limit=100&agent=codex",
      body: undefined,
    });
  });
});
