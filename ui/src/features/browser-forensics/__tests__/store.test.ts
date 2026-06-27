import { describe, it, expect, beforeEach } from "vitest";
import {
  useBrowserForensicsStore,
  urlToHostname,
  hostnameMatchesDomain,
  shouldUpgradeToConfirmed,
} from "../store";
import type { EvidenceObject, ExtensionAttributionPayload, ExtensionAttributionSummary } from "../types";

// ── 测试用 mock 工厂 ─────────────────────────────────────────────

function makeAttributionEvent(
  overrides: Partial<ExtensionAttributionPayload> = {},
): ExtensionAttributionPayload {
  return {
    timestamp: Date.now(),
    request_id: "req-1",
    url: "https://evil.com/path",
    method: "GET",
    initiator: null,
    attribution_status: "matched",
    extension_id: "ext-1",
    extension_name: "EvilExt",
    level: "confirmed",
    ...overrides,
  };
}

function makeExtAttr(
  confidence: ExtensionAttributionSummary["confidence"] = "possible",
): ExtensionAttributionSummary {
  return { confidence, matched: [] };
}

/// 构造最小可用 EvidenceObject（仅 domain + extension_attribution 被 action 访问）
function makeEvidence(
  domain: string,
  extAttr: ExtensionAttributionSummary | null = makeExtAttr("possible"),
): EvidenceObject {
  return {
    domain,
    process: "chrome.exe",
    pid: 1234,
    alert_id: null,
    malicious_connection: {
      domain,
      ip: "1.2.3.4",
      process: "chrome.exe",
      pid: 1234,
      browser: "chrome",
      profile: "Default",
      timestamp: "2026-06-26T00:00:00Z",
    },
    history_correlation: null,
    downloads: [],
    navigation_chain: [],
    extension_attribution: extAttr,
    tab_attribution: null,
    overall_confidence: "possible",
    overall_score: 0,
  };
}

// ── urlToHostname ────────────────────────────────────────────────

describe("urlToHostname", () => {
  it("extracts hostname from normal URL", () => {
    expect(urlToHostname("https://evil.com/path")).toBe("evil.com");
    expect(urlToHostname("http://sub.evil.com:8080/x")).toBe("sub.evil.com");
  });

  it("returns null for URL without protocol", () => {
    expect(urlToHostname("evil.com")).toBeNull();
    expect(urlToHostname("/just/a/path")).toBeNull();
  });

  it("returns null for invalid URL", () => {
    expect(urlToHostname("")).toBeNull();
    expect(urlToHostname("not a url at all")).toBeNull();
  });
});

// ── hostnameMatchesDomain ────────────────────────────────────────

describe("hostnameMatchesDomain", () => {
  it("matches exact domain", () => {
    expect(hostnameMatchesDomain("evil.com", "evil.com")).toBe(true);
  });

  it("matches subdomain via suffix", () => {
    expect(hostnameMatchesDomain("sub.evil.com", "evil.com")).toBe(true);
    expect(hostnameMatchesDomain("a.b.evil.com", "evil.com")).toBe(true);
  });

  it("does not match substring trap (notevil.com)", () => {
    expect(hostnameMatchesDomain("notevil.com", "evil.com")).toBe(false);
  });

  it("does not match unrelated domain", () => {
    expect(hostnameMatchesDomain("safe.com", "evil.com")).toBe(false);
  });

  it("does not match empty hostname", () => {
    expect(hostnameMatchesDomain("", "evil.com")).toBe(false);
  });
});

// ── shouldUpgradeToConfirmed ─────────────────────────────────────

describe("shouldUpgradeToConfirmed", () => {
  it("returns true when confirmed event hostname matches target domain", () => {
    const events = [makeAttributionEvent({ url: "https://evil.com/x", level: "confirmed" })];
    expect(shouldUpgradeToConfirmed(events, "evil.com")).toBe(true);
  });

  it("returns true when confirmed event hostname is subdomain of target", () => {
    const events = [makeAttributionEvent({ url: "https://sub.evil.com/x", level: "confirmed" })];
    expect(shouldUpgradeToConfirmed(events, "evil.com")).toBe(true);
  });

  it("returns false when confirmed event hostname does not match target", () => {
    const events = [makeAttributionEvent({ url: "https://safe.com/x", level: "confirmed" })];
    expect(shouldUpgradeToConfirmed(events, "evil.com")).toBe(false);
  });

  it("returns false when event is probable (not confirmed) even if hostname matches", () => {
    const events = [makeAttributionEvent({ url: "https://evil.com/x", level: "probable" })];
    expect(shouldUpgradeToConfirmed(events, "evil.com")).toBe(false);
  });

  it("returns false when no events", () => {
    expect(shouldUpgradeToConfirmed([], "evil.com")).toBe(false);
  });

  it("does not fall for notevil.com substring trap", () => {
    const events = [makeAttributionEvent({ url: "https://notevil.com/x", level: "confirmed" })];
    expect(shouldUpgradeToConfirmed(events, "evil.com")).toBe(false);
  });
});

// ── upgradeContextExtensionConfidence (store action) ────────────

describe("upgradeContextExtensionConfidence", () => {
  beforeEach(() => {
    useBrowserForensicsStore.setState({
      contextResult: null,
      extensionAttributions: [],
    });
  });

  it("upgrades confidence to confirmed when domain matches", () => {
    useBrowserForensicsStore.setState({ contextResult: makeEvidence("evil.com", makeExtAttr("possible")) });
    useBrowserForensicsStore.getState().upgradeContextExtensionConfidence("evil.com");
    const ctx = useBrowserForensicsStore.getState().contextResult!;
    expect(ctx.extension_attribution!.confidence).toBe("confirmed");
  });

  it("does not upgrade when domain does not match", () => {
    useBrowserForensicsStore.setState({ contextResult: makeEvidence("evil.com", makeExtAttr("possible")) });
    useBrowserForensicsStore.getState().upgradeContextExtensionConfidence("other.com");
    expect(useBrowserForensicsStore.getState().contextResult!.extension_attribution!.confidence).toBe("possible");
  });

  it("does not upgrade when extension_attribution is null", () => {
    useBrowserForensicsStore.setState({ contextResult: makeEvidence("evil.com", null) });
    useBrowserForensicsStore.getState().upgradeContextExtensionConfidence("evil.com");
    expect(useBrowserForensicsStore.getState().contextResult!.extension_attribution).toBeNull();
  });

  it("does not upgrade when contextResult is null", () => {
    useBrowserForensicsStore.getState().upgradeContextExtensionConfidence("evil.com");
    expect(useBrowserForensicsStore.getState().contextResult).toBeNull();
  });

  it("is idempotent when already confirmed", () => {
    useBrowserForensicsStore.setState({ contextResult: makeEvidence("evil.com", makeExtAttr("confirmed")) });
    const before = useBrowserForensicsStore.getState().contextResult;
    useBrowserForensicsStore.getState().upgradeContextExtensionConfidence("evil.com");
    // 引用未变（无重复升级）
    expect(useBrowserForensicsStore.getState().contextResult).toBe(before);
  });
});

// ── watchTargets (store actions) ─────────────────────────────────

describe("watchTargets", () => {
  beforeEach(() => {
    useBrowserForensicsStore.setState({ watchTargets: [] });
  });

  it("addWatchTarget adds new target", () => {
    useBrowserForensicsStore.getState().addWatchTarget("ip138.com");
    expect(useBrowserForensicsStore.getState().watchTargets).toEqual(["ip138.com"]);
  });

  it("addWatchTarget ignores empty string", () => {
    useBrowserForensicsStore.getState().addWatchTarget("  ");
    expect(useBrowserForensicsStore.getState().watchTargets).toEqual([]);
  });

  it("addWatchTarget ignores duplicates", () => {
    useBrowserForensicsStore.getState().addWatchTarget("ip138.com");
    useBrowserForensicsStore.getState().addWatchTarget("ip138.com");
    expect(useBrowserForensicsStore.getState().watchTargets).toEqual(["ip138.com"]);
  });

  it("addWatchTarget trims whitespace", () => {
    useBrowserForensicsStore.getState().addWatchTarget("  ip138.com  ");
    expect(useBrowserForensicsStore.getState().watchTargets).toEqual(["ip138.com"]);
  });

  it("removeWatchTarget removes target", () => {
    useBrowserForensicsStore.getState().addWatchTarget("ip138.com");
    useBrowserForensicsStore.getState().addWatchTarget("evil.com");
    useBrowserForensicsStore.getState().removeWatchTarget("ip138.com");
    expect(useBrowserForensicsStore.getState().watchTargets).toEqual(["evil.com"]);
  });

  it("clearWatchTargets empties list", () => {
    useBrowserForensicsStore.getState().addWatchTarget("ip138.com");
    useBrowserForensicsStore.getState().clearWatchTargets();
    expect(useBrowserForensicsStore.getState().watchTargets).toEqual([]);
  });
});
