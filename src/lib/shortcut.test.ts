import { describe, expect, it } from "vitest";
import {
  acceleratorToDisplay,
  acceleratorToTauri,
  isValidAccelerator,
  normalizeAccelerator,
  type AcceleratorKeyEvent,
} from "./shortcut";

const key = (overrides: Partial<AcceleratorKeyEvent>): AcceleratorKeyEvent => ({
  ctrlKey: false,
  shiftKey: false,
  altKey: false,
  metaKey: false,
  key: "",
  ...overrides,
});

describe("normalizeAccelerator", () => {
  it("单键：字母无修饰键时返回大写字母", () => {
    expect(normalizeAccelerator(key({ key: "a" }))).toBe("A");
  });

  it("单键：数字与 F 键", () => {
    expect(normalizeAccelerator(key({ key: "5" }))).toBe("5");
    expect(normalizeAccelerator(key({ key: "F5" }))).toBe("F5");
  });

  it("组合键：Ctrl+Shift+N", () => {
    expect(normalizeAccelerator(key({ ctrlKey: true, shiftKey: true, key: "n" }))).toBe(
      "Ctrl+Shift+N",
    );
  });

  it("metaKey 映射为 Super", () => {
    expect(normalizeAccelerator(key({ metaKey: true, key: "q" }))).toBe("Super+Q");
  });

  it("四个修饰键按 Ctrl/Shift/Alt/Super 排序", () => {
    expect(
      normalizeAccelerator(
        key({ ctrlKey: true, shiftKey: true, altKey: true, metaKey: true, key: "x" }),
      ),
    ).toBe("Ctrl+Shift+Alt+Super+X");
  });

  it("仅按下修饰键时返回 null", () => {
    expect(normalizeAccelerator(key({ key: "Control", ctrlKey: true }))).toBeNull();
    expect(normalizeAccelerator(key({ key: "Shift", shiftKey: true }))).toBeNull();
    expect(normalizeAccelerator(key({ key: "Alt", altKey: true }))).toBeNull();
    expect(normalizeAccelerator(key({ key: "Meta", metaKey: true }))).toBeNull();
  });

  it("空格归一为 Space", () => {
    expect(normalizeAccelerator(key({ ctrlKey: true, key: " " }))).toBe("Ctrl+Space");
  });
});

describe("acceleratorToDisplay", () => {
  it("大小写归一为标准显示形式", () => {
    expect(acceleratorToDisplay("ctrl+shift+n")).toBe("Ctrl+Shift+N");
    expect(acceleratorToDisplay("CTRL+A")).toBe("Ctrl+A");
    expect(acceleratorToDisplay("super+q")).toBe("Super+Q");
    expect(acceleratorToDisplay("alt+f4")).toBe("Alt+F4");
    expect(acceleratorToDisplay("ctrl+space")).toBe("Ctrl+Space");
  });
});

describe("acceleratorToTauri", () => {
  it("整体小写化供 Tauri 解析", () => {
    expect(acceleratorToTauri("Ctrl+Shift+N")).toBe("ctrl+shift+n");
    expect(acceleratorToTauri("Super+Q")).toBe("super+q");
  });
});

describe("isValidAccelerator", () => {
  it("有效：至少一个修饰键 + 一个非修饰键主键", () => {
    expect(isValidAccelerator("Ctrl+A")).toBe(true);
    expect(isValidAccelerator("ctrl+alt+delete")).toBe(true);
    expect(isValidAccelerator("Super+Space")).toBe(true);
  });

  it("无效：无修饰键、无主键、多个主键、空段", () => {
    expect(isValidAccelerator("A")).toBe(false);
    expect(isValidAccelerator("Ctrl")).toBe(false);
    expect(isValidAccelerator("Ctrl+Shift")).toBe(false);
    expect(isValidAccelerator("Ctrl+A+B")).toBe(false);
    expect(isValidAccelerator("Ctrl+")).toBe(false);
    expect(isValidAccelerator("+A")).toBe(false);
    expect(isValidAccelerator("")).toBe(false);
  });
});
