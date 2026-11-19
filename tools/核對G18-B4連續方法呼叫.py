"""核對連續同步方法的完整案例清單，逐項執行 Rust 測試。"""
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_repeated_self_call_tests::"
CASES = (
    "普通函式持有雙實例字串時成功及溢位皆逆序清理",
    "普通函式逐框架返回方法值且溢位逐框架清理",
    "逐次借票收據與整段工作圖必須一致",
    "普通函式轉交具名整數形參且同名槽分屬不同框架",
    "另一持有者溢位時已還票且其後的持有者不再執行",
    "交替持有者共用工作圖但保有各自欄位與借票",
    "呼叫之間保留初始化與空槽並只執行一次陳述式",
    "兩次呼叫共用布林欄位且每次使用自己的借票",
    "連續方法回傳槽供後續實參與共享讀取",
    "中間方法溢位時不再簽發後續呼叫票據",
    "前置呼叫順序遭變造時整批拒絕",
    "相同方法的兩份成功結果對調或遺失仍會拒絕",
)


def run(*args):
    return subprocess.check_output([sys.argv[1], *args], encoding="utf-8")


if __name__ == "__main__":
    cfg_prefix = "struct_stage::g18_scalar_self_cfg_tests::"
    cfg_cases = (
        "布林實參在方法內決定提前返回的欄位",
        "方法內算術溢位先還獨占票再清理呼叫者",
        "迴圈實參以當下值綁入方法形參",
        "迴圈內獨占方法寫入在歸票後仍可共享讀取",
        "建構具名實參的方法讀取依序還票且溢位不提交",
        "同一實參序列中普通字串借票與方法票獨立歸還",
        "方法來源收據與重播票據分別拒絕",
        "普通函式的迴圈與提前返回依動態次序借票",
        "步數耗盡仍清理持有者且不重播方法",
    )
    groups = {PREFIX: CASES, cfg_prefix: cfg_cases}
    expected = sorted(prefix + case for prefix, cases in groups.items() for case in cases)
    found = sorted(line.removesuffix(": test") for line in run("--list").splitlines()
                   if line.startswith(tuple(groups)))
    if found != expected:
        sys.exit(f"連續方法測試清單與預期不同 {found}")
    for name in expected:
        result = run("--exact", name, "--test-threads=1")
        if not re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", result):
            sys.exit(result)
        print(name)
