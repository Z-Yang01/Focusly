/** 行级 diff（LCS 动态规划）— 纯函数、零依赖；便签规模 O(n·m) 足够 */

export type DiffLineKind = "same" | "add" | "remove";

export interface DiffLine {
  kind: DiffLineKind;
  text: string;
  /** 在 a（旧文本）中的 0-based 行号；kind 为 "add" 时无 */
  aLine?: number;
  /** 在 b（新文本）中的 0-based 行号；kind 为 "remove" 时无 */
  bLine?: number;
}

/**
 * 行级 LCS diff。
 * 统一按 split("\n") 切行："" → [""]、"a\n" → ["a", ""]，
 * 因此空串按一行空行处理，结尾换行差异体现为一行空行的新增/删除。
 */
export function diffLines(a: string, b: string): DiffLine[] {
  const A = a.split("\n");
  const B = b.split("\n");
  const n = A.length;
  const m = B.length;

  // dp[i][j] = A[i..] 与 B[j..] 的最长公共子序列长度（带哨兵行列，边界为 0）
  const dp: Uint32Array[] = new Array(n + 1);
  for (let i = 0; i <= n; i++) dp[i] = new Uint32Array(m + 1);
  for (let i = n - 1; i >= 0; i--) {
    const row = dp[i];
    const next = dp[i + 1];
    for (let j = m - 1; j >= 0; j--) {
      row[j] = A[i] === B[j] ? next[j + 1] + 1 : Math.max(next[j], row[j + 1]);
    }
  }

  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (A[i] === B[j]) {
      out.push({ kind: "same", text: A[i], aLine: i, bLine: j });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      // 打平时优先输出删除，保持 "remove 在前、add 在后" 的阅读顺序
      out.push({ kind: "remove", text: A[i], aLine: i });
      i++;
    } else {
      out.push({ kind: "add", text: B[j], bLine: j });
      j++;
    }
  }
  for (; i < n; i++) out.push({ kind: "remove", text: A[i], aLine: i });
  for (; j < m; j++) out.push({ kind: "add", text: B[j], bLine: j });
  return out;
}

/** 统计新增 / 删除行数 */
export function diffStats(diff: DiffLine[]): { added: number; removed: number } {
  let added = 0;
  let removed = 0;
  for (const line of diff) {
    if (line.kind === "add") added++;
    else if (line.kind === "remove") removed++;
  }
  return { added, removed };
}
