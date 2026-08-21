import { describe, expect, it } from "vitest";
import { p2pErrorMessage } from "./index";

describe("p2pErrorMessage", () => {
  it("resolves a typed backend category through the active locale", () => {
    expect(p2pErrorMessage("authorization_denied")).toBe("连接授权被拒绝");
  });
});
