# voiles

voiles 是開發中的自製程式語言，目標是結合 Python 風格的簡潔編寫方式，以及參考 Rust 的記憶體安全與效能設計。

## 專案方向

- **主要執行模式**：直譯執行。
- **延伸能力**：編譯為原生程式，以及打包為可獨立執行的檔案。
- **長期目標**：直譯器與編譯器逐步以 voiles 自身實作，完成自舉。
- **型別系統**：強型別、靜態型別、變數強制標註型別。
- **記憶體管理**：所有權、明確移動與借用檢查；預設不依賴垃圾回收。

## 語法方向

以下是目前確認的語法示意，**不是已可執行的程式**：

```python
var x:int = 5
const message:str = "voiles"
share count:int = 0

var a:str = "Hello"
var b:str = a
var reader:str = borrow(a)
var transferred:str = move(b)
```

一般賦值預設複製（前提是型別支援複製）；明確使用 `move(...)` 轉移所有權，以 `borrow(...)` 或 `borrow_mut(...)` 建立借用。

## 目前狀態

本儲存庫目前處於**語言規格與開發流程初始化**階段，尚未提供可使用的直譯器、編譯器或發行版本。

核心語法、容器及資源管理的完整細節仍在逐步確認。此時不提供尚未實作的安裝或執行命令。

## 文件與協作

本檔面向外部使用者；內部規格、工作紀錄、已確認決策與待確認議題位於 `AI-build/`。所有說明文件以繁體中文撰寫，檔案使用 UTF-8 編碼。

專案位置：[siyujellyfish/voiles](https://github.com/siyujellyfish/voiles)。
