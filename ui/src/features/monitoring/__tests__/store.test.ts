import { describe, it, expect, beforeEach } from "vitest";
import { useMonitoringStore } from "../store";

describe("useMonitoringStore", () => {
  beforeEach(() => {
    // Reset store to initial state before each test
    useMonitoringStore.setState({
      isBackground: false,
      telemetry: null,
      eventCount: 0,
    });
  });

  it("has correct initial state", () => {
    const state = useMonitoringStore.getState();
    expect(state.isBackground).toBe(false);
    expect(state.telemetry).toBeNull();
    expect(state.eventCount).toBe(0);
  });

  it("setIsBackground updates isBackground", () => {
    useMonitoringStore.getState().setIsBackground(true);
    expect(useMonitoringStore.getState().isBackground).toBe(true);

    useMonitoringStore.getState().setIsBackground(false);
    expect(useMonitoringStore.getState().isBackground).toBe(false);
  });

  it("setTelemetry updates telemetry", () => {
    const telemetry = {
      mode: "Background",
      started_at: Date.now(),
      events_written: 42,
      events_dropped: 1,
      last_event_at: Date.now(),
      last_error: null,
    };
    useMonitoringStore.getState().setTelemetry(telemetry);
    expect(useMonitoringStore.getState().telemetry).toEqual(telemetry);
  });

  it("setTelemetry can set null", () => {
    const telemetry = {
      mode: "Foreground",
      started_at: 1000,
      events_written: 0,
      events_dropped: 0,
      last_event_at: null,
      last_error: null,
    };
    useMonitoringStore.getState().setTelemetry(telemetry);
    expect(useMonitoringStore.getState().telemetry).toEqual(telemetry);

    useMonitoringStore.getState().setTelemetry(null);
    expect(useMonitoringStore.getState().telemetry).toBeNull();
  });

  it("setEventCount updates eventCount", () => {
    useMonitoringStore.getState().setEventCount(100);
    expect(useMonitoringStore.getState().eventCount).toBe(100);

    useMonitoringStore.getState().setEventCount(0);
    expect(useMonitoringStore.getState().eventCount).toBe(0);
  });
});
