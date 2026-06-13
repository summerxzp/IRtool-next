import { describe, it, expect, vi } from "vitest";
import { formatUptime, formatTimestamp } from "../utils";

describe("formatUptime", () => {
  it("returns '-' for null", () => {
    expect(formatUptime(null)).toBe("-");
  });

  it("returns seconds for durations less than 1 minute", () => {
    const now = Date.now();
    vi.useFakeTimers();
    vi.setSystemTime(now);

    // 5 seconds ago
    expect(formatUptime(now - 5_000)).toBe("5秒");
    // 0 seconds ago (just started)
    expect(formatUptime(now)).toBe("0秒");
    // 59 seconds ago
    expect(formatUptime(now - 59_000)).toBe("59秒");

    vi.useRealTimers();
  });

  it("returns minutes and seconds for durations between 1 minute and 1 hour", () => {
    const now = Date.now();
    vi.useFakeTimers();
    vi.setSystemTime(now);

    // 1 minute 30 seconds
    expect(formatUptime(now - 90_000)).toBe("1分30秒");
    // 5 minutes 0 seconds
    expect(formatUptime(now - 300_000)).toBe("5分0秒");
    // 59 minutes 59 seconds
    expect(formatUptime(now - 59 * 60_000 - 59_000)).toBe("59分59秒");

    vi.useRealTimers();
  });

  it("returns hours and minutes for durations between 1 hour and 1 day", () => {
    const now = Date.now();
    vi.useFakeTimers();
    vi.setSystemTime(now);

    // 1 hour 0 minutes
    expect(formatUptime(now - 3_600_000)).toBe("1时0分");
    // 2 hours 30 minutes
    expect(formatUptime(now - 2 * 3_600_000 - 30 * 60_000)).toBe("2时30分");
    // 23 hours 59 minutes
    expect(formatUptime(now - 23 * 3_600_000 - 59 * 60_000)).toBe("23时59分");

    vi.useRealTimers();
  });

  it("returns days and hours for durations of 1 day or more", () => {
    const now = Date.now();
    vi.useFakeTimers();
    vi.setSystemTime(now);

    // 1 day 0 hours
    expect(formatUptime(now - 86_400_000)).toBe("1天0时");
    // 1 day 5 hours
    expect(formatUptime(now - 86_400_000 - 5 * 3_600_000)).toBe("1天5时");
    // 3 days 12 hours
    expect(formatUptime(now - 3 * 86_400_000 - 12 * 3_600_000)).toBe("3天12时");

    vi.useRealTimers();
  });
});

describe("formatTimestamp", () => {
  it("returns '-' for null", () => {
    expect(formatTimestamp(null)).toBe("-");
  });

  it("formats a known timestamp to zh-CN locale string", () => {
    // 2024-01-15 08:30:00 UTC → check that it returns a non-empty string
    const ts = new Date("2024-01-15T08:30:00.000Z").getTime();
    const result = formatTimestamp(ts);
    expect(result).not.toBe("-");
    expect(result.length).toBeGreaterThan(0);
  });

  it("returns a string containing the year for a valid timestamp", () => {
    const ts = new Date("2024-06-01T12:00:00.000Z").getTime();
    const result = formatTimestamp(ts);
    expect(result).toContain("2024");
  });
});
