import { readFileSync } from "node:fs";
import vm from "node:vm";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const html = readFileSync(
  new URL("../../src-tauri/mcp-app/delivery.html", import.meta.url),
  "utf8",
);
const script = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)].slice(
  -1,
)[0][1];
type Result = {
  content?: { type: string; text?: string; data?: string; mimeType?: string }[];
  structuredContent?: Record<string, unknown>;
  isError?: boolean;
};
class Element {
  hidden = false;
  textContent = "";
  className = "";
  src = "";
  alt = "";
  open = false;
  style: Record<string, string> = {};
  children: Element[] = [];
  attributes: Record<string, string> = {};
  classes = new Set<string>();
  classList = {
    toggle: (name: string, on: boolean) =>
      on ? this.classes.add(name) : this.classes.delete(name),
  };
  setAttribute(name: string, value: string) {
    this.attributes[name] = value;
  }
  append(...children: Element[]) {
    this.children.push(...children);
  }
  appendChild(child: Element) {
    this.append(child);
  }
  replaceChildren(...children: Element[]) {
    this.children = children;
  }
  showModal() {
    this.open = true;
  }
  close() {
    this.open = false;
  }
}
const delivery = (seq = 12): Result => ({
  content: [{ type: "image", mimeType: "image/jpeg", data: "AA==" }],
  structuredContent: {
    kind: "delivery",
    seq,
    session: { id: "s" },
    deliveryKind: "single",
    images: [{ cameraName: "Desk", timestampMs: 1000 }],
    status: { receivedAtMs: 2000, work: null },
  },
});
async function host(
  tool = "wait_for_delivery",
  fallback: () => Promise<Result> = async () => ({
    structuredContent: { kind: "waiting" },
  }),
) {
  const elements = new Map<string, Element>();
  const el = (id: string) => {
    if (!elements.has(id)) elements.set(id, new Element());
    return elements.get(id)!;
  };
  class App {
    static instance: App;
    constructor() {
      App.instance = this;
    }
    ontoolinput = (_input: { arguments?: Record<string, unknown> }) => {};
    ontoolresult = (_result: Result) => {};
    ontoolcancelled = () => {};
    onteardown = () => ({});
    callServerTool = vi.fn(fallback);
    connect = () => Promise.resolve();
    getHostContext = () => ({ toolInfo: { tool: { name: tool } } });
  }
  vm.runInNewContext(script, {
    ExtApps: {
      App,
      applyDocumentTheme() {},
      applyHostStyleVariables() {},
      applyHostFonts() {},
    },
    document: {
      getElementById: el,
      createElement: () => new Element(),
      body: new Element(),
    },
    setTimeout,
    clearTimeout,
    Date,
    console,
  });
  await Promise.resolve();
  return { app: App.instance, el };
}
beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("MCP card lifecycle with a simulated host", () => {
  it("replaces waiting with receipt without claiming execution", async () => {
    const { app, el } = await host();
    app.ontoolinput({ arguments: { after_seq: 11 } });
    app.ontoolresult(delivery());
    expect(el("status-label").textContent).toBe("Delivered to agent");
    expect(el("status").classes.has("status--pulsing")).toBe(false);
    expect(el("result").hidden).toBe(false);
  });
  it("renders concise timeout and hides old images", async () => {
    const { app, el } = await host();
    app.ontoolresult(delivery());
    app.ontoolresult({
      structuredContent: { kind: "timeout" },
      content: [{ type: "text", text: "after_seq=12" }],
    });
    expect(el("status-label").textContent).not.toContain("after_seq");
    expect(el("result").hidden).toBe(true);
  });
  it("preserves exact lookup arguments through an app-only fallback", async () => {
    const { app } = await host("get_delivery", async () => delivery(9));
    app.ontoolinput({ arguments: { seq: 9 } });
    await vi.advanceTimersByTimeAsync(2500);
    expect(app.callServerTool).toHaveBeenCalledWith(
      { name: "get_card_state", arguments: { tool: "get_delivery", seq: 9 } },
      { timeout: 8000 },
    );
  });
  it("never resurrects a cancelled call when an in-flight poll completes", async () => {
    let resolve!: (result: Result) => void;
    const pending = new Promise<Result>((done) => {
      resolve = done;
    });
    const { app, el } = await host("get_latest_delivery", () => pending);
    app.ontoolinput({ arguments: {} });
    await vi.advanceTimersByTimeAsync(2500);
    app.ontoolcancelled();
    resolve(delivery());
    await Promise.resolve();
    expect(el("status-label").textContent).toContain("Cancelled");
    expect(el("result").hidden).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
  });
  it("ends bounded fallback polling visibly", async () => {
    const { app, el } = await host();
    app.ontoolinput({ arguments: {} });
    await vi.advanceTimersByTimeAsync(220000);
    expect(el("status-label").textContent).toContain("unconfirmed");
    expect(vi.getTimerCount()).toBe(0);
  });
  it("ignores a late host result after cancellation", async () => {
    const { app, el } = await host();
    app.ontoolcancelled();
    app.ontoolresult(delivery());
    expect(el("status-label").textContent).toContain("Cancelled");
  });
  it("shows acknowledged work and stops polling on completion", async () => {
    const result: Result = {
      structuredContent: {
        kind: "status",
        seq: 12,
        session: { id: "s" },
        status: { work: { state: "completed", summary: "Explanation ready" } },
      },
    };
    const { app, el } = await host("wait_for_delivery", async () => result);
    app.ontoolresult(delivery());
    await vi.advanceTimersByTimeAsync(3000);
    expect(el("status-label").textContent).toBe("Completed");
    expect(el("status-detail").textContent).toBe("Explanation ready");
    expect(el("result").hidden).toBe(false);
    expect(vi.getTimerCount()).toBe(0);
  });
  it("shows pause as a terminal outcome", async () => {
    const { app, el } = await host("wait_for_delivery", async () => ({
      structuredContent: { kind: "paused" },
    }));
    app.ontoolinput({ arguments: { after_seq: 11 } });
    await vi.advanceTimersByTimeAsync(2500);
    expect(el("status-label").textContent).toContain("Paused in Tafel");
    expect(vi.getTimerCount()).toBe(0);
  });
  it("does not claim an agent receipt for a fallback-only image", async () => {
    const result = delivery();
    result.structuredContent!.status = { work: null, receivedAtMs: null };
    const { app, el } = await host("get_latest_delivery", async () => result);
    app.ontoolinput({ arguments: {} });
    await vi.advanceTimersByTimeAsync(2500);
    expect(el("status-label").textContent).toContain("receipt unconfirmed");
    expect(el("result").hidden).toBe(false);
  });
  it("stops all timers at teardown", async () => {
    const { app } = await host();
    app.ontoolresult(delivery());
    app.onteardown();
    expect(vi.getTimerCount()).toBe(0);
  });
});
