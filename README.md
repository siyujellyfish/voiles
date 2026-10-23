# voiles

voiles 是規劃中的自製程式語言。本儲存庫目前以語言設計與開發規劃為主，**尚未提供可執行的直譯器或編譯器**。

## 新版語言設計方向（初步草案）

自 2026-10-23 起以新版草案為準，暫捨棄此前版本的具體語法規格。新版目前包含：

- 型別：`int`、`float`、`str`、`bool`、`list[T]`、`dict{K,V}`、`none`，以及具多型別聯集／缺失值語意的 `Optional`。
- 大括號 `{}` 劃定作用域；`var`、`share`、`const` 宣告資料。
- 函式 `fn` 預設公開；`_` 前綴函式為內部函式，不允許跨模組呼叫。
- 結構 `struct`、模組 `import`、條件、迴圈、比對、`try/except` 與 `async fn/await`。
- 值的預設複製、明確 `.move()` 與 `.borrow()`；字串依字元及其 UTF-8 位元組雙層索引。

以下僅為設計語法示意，**目前不可執行**：

```voiles
# a.voil
share count:int = 5

fn read_count()->int{
    return count
}

fn _helper()->none{
    # 模組內部函式
}
```

```voiles
# main.voil
import a

fn main()->none{
    print(a.read_count())  # 公開函式
    a._helper()            # 錯誤：不能跨模組呼叫內部函式
}
```

## 專案狀態

新版語言規格仍有待決細節；程式碼實作、編譯模式、開發工具及發行方式均需以後續已核准且已驗證的成果為準。

內部規格、待確認議題及工作階段記錄於 [AI-build](AI-build/README.md)。所有文件採繁體中文與 UTF-8。

儲存庫：[siyujellyfish/voiles](https://github.com/siyujellyfish/voiles)。
