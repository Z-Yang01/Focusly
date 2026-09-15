/** 勿扰时段纯逻辑单测（对应 src-tauri/src/dnd.rs 的同规则 Rust 单测） */
import { describe, expect, it } from "vitest";
import {
  DEFAULT_END,
  DEFAULT_START,
  isCrossMidnight,
  isDndEnabled,
  isValidHm,
} from "./dnd";

describe("isValidHm", () => {
  it("接受合法 HH:MM", () => {
    expect(isValidHm("00:00")).toBe(true);
    expect(isValidHm("07:00")).toBe(true);
    expect(isValidHm("22:30")).toBe(true);
    expect(isValidHm("23:59")).toBe(true);
  });

  it("拒绝空串与缺失", () => {
    expect(isValidHm("")).toBe(false);
  });

  it("拒绝非两位与越界", () => {
    expect(isValidHm("7:00")).toBe(false);
    expect(isValidHm("22:5")).toBe(false);
    expect(isValidHm("24:00")).toBe(false);
    expect(isValidHm("12:60")).toBe(false);
  });

  it("拒绝非数字与非冒号分隔", () => {
    expect(isValidHm("ab:cd")).toBe(false);
    expect(isValidHm("22-00")).toBe(false);
    expect(isValidHm("2200")).toBe(false);
  });
});

describe("isDndEnabled", () => {
  it("起止均合法且不同 = 启用", () => {
    expect(isDndEnabled("22:00", "07:00")).toBe(true);
    expect(isDndEnabled("13:00", "15:00")).toBe(true);
  });

  it("start==end 视为未启用（与 Rust parse_window 一致）", () => {
    expect(isDndEnabled("13:00", "13:00")).toBe(false);
    expect(isDndEnabled("00:00", "00:00")).toBe(false);
  });

  it("任一端非法/为空 = 未启用", () => {
    expect(isDndEnabled("", "")).toBe(false);
    expect(isDndEnabled("", "07:00")).toBe(false);
    expect(isDndEnabled("22:00", "")).toBe(false);
    expect(isDndEnabled("25:00", "07:00")).toBe(false);
  });
});

describe("isCrossMidnight", () => {
  it("start > end 即跨午夜", () => {
    expect(isCrossMidnight("22:00", "07:00")).toBe(true);
    expect(isCrossMidnight("23:59", "00:00")).toBe(true);
  });

  it("常规窗口与未启用均不算跨午夜", () => {
    expect(isCrossMidnight("13:00", "15:00")).toBe(false);
    expect(isCrossMidnight("13:00", "13:00")).toBe(false);
    expect(isCrossMidnight("", "")).toBe(false);
  });
});

describe("默认值", () => {
  it("默认窗口本身合法且跨午夜", () => {
    expect(isValidHm(DEFAULT_START)).toBe(true);
    expect(isValidHm(DEFAULT_END)).toBe(true);
    expect(isDndEnabled(DEFAULT_START, DEFAULT_END)).toBe(true);
    expect(isCrossMidnight(DEFAULT_START, DEFAULT_END)).toBe(true);
  });
});
