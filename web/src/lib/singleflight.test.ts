import { describe, expect, it, vi } from "vitest";
import { SingleFlight } from "./singleflight";

/** A promise plus the handles to settle it, so a test controls when work finishes. */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("SingleFlight", () => {
  it("runs one operation for callers that overlap", async () => {
    const flight = new SingleFlight();
    const work = deferred<string>();
    const operation = vi.fn(() => work.promise);

    const first = flight.run("key", operation);
    const second = flight.run("key", operation);
    work.resolve("value");

    expect(await first).toBe("value");
    expect(await second).toBe("value");
    expect(operation).toHaveBeenCalledTimes(1);
  });

  it("runs again once the first has settled", async () => {
    const flight = new SingleFlight();
    const operation = vi.fn(() => Promise.resolve("value"));

    await flight.run("key", operation);
    await flight.run("key", operation);

    expect(operation).toHaveBeenCalledTimes(2);
  });

  it("keeps different keys independent", async () => {
    const flight = new SingleFlight();
    const work = deferred<string>();
    const operation = vi.fn(() => work.promise);

    const both = Promise.all([flight.run("a", operation), flight.run("b", operation)]);
    work.resolve("value");
    await both;

    expect(operation).toHaveBeenCalledTimes(2);
  });

  it("hands the same failure to everyone who joined", async () => {
    const flight = new SingleFlight();
    const work = deferred<string>();
    const operation = vi.fn(() => work.promise);
    const failure = new Error("upstream");

    const first = flight.run("key", operation);
    const second = flight.run("key", operation);
    work.reject(failure);

    await expect(first).rejects.toBe(failure);
    await expect(second).rejects.toBe(failure);
    expect(operation).toHaveBeenCalledTimes(1);
  });

  it("lets the next caller retry after a failure", async () => {
    const flight = new SingleFlight();
    const operation = vi.fn(() => Promise.reject(new Error("upstream")));

    await expect(flight.run("key", operation)).rejects.toThrow("upstream");
    await expect(flight.run("key", operation)).rejects.toThrow("upstream");

    expect(operation).toHaveBeenCalledTimes(2);
  });

  it("does not let a synchronous throw strand the key", async () => {
    const flight = new SingleFlight();
    const operation = vi.fn(() => {
      throw new Error("built the request wrong");
    });

    expect(() => flight.run("key", operation)).toThrow("built the request wrong");

    await expect(flight.run("key", () => Promise.resolve("value"))).resolves.toBe("value");
  });

  it("shares nothing between separate instances", async () => {
    const work = deferred<string>();
    const operation = vi.fn(() => work.promise);

    const both = Promise.all([
      new SingleFlight().run("key", operation),
      new SingleFlight().run("key", operation),
    ]);
    work.resolve("value");
    await both;

    expect(operation).toHaveBeenCalledTimes(2);
  });
});
