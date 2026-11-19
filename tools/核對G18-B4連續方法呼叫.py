"""核對連續同步方法的完整案例清單，逐項執行 Rust 測試。"""
import re
import subprocess
import sys

PREFIX = "struct_stage::g18_repeated_self_call_tests::"
CASES = (
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
    expected = sorted(PREFIX + case for case in CASES)
    found = sorted(line.removesuffix(": test") for line in run("--list").splitlines()
                   if line.startswith(PREFIX))
    if found != expected:
        sys.exit(f"連續方法測試清單與預期不同 {found}")
    for name in expected:
        result = run("--exact", name, "--test-threads=1")
        if not re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", result):
            sys.exit(result)
        print(name)
