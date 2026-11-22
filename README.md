# voiles

voiles 是採明確型別、所有權與借用規則的程式語言。主線可在 Linux x86-64 GNU 上檢查、直譯、測試多檔專案，並產生原生執行檔。編譯工具鏈使用 Rust 宿主及可自舉的 voiles 元件。

## 從主線建置並執行

以下命令從儲存庫根目錄執行，需要 stable Rust。bootstrap 工具會建立新的輸出目錄。

```sh
python3 tools/bootstrap.py target/dev-cli
cli=./target/dev-cli/voiles
"$cli" check examples/詞數統計/main.voil
"$cli" run examples/詞數統計/main.voil -- examples/詞數統計/sample.txt
"$cli" test examples/詞數統計
```

詞數工具先印出報表 `3`，再由 `run` 顯示 main 返回值 `0`。專案測試會執行目錄裡的 `test.voil`，以完整整數返回值判斷成敗。亦可在專案內直接執行 `voiles test`。[詞數統計範例](examples/詞數統計)

## 建置與交付

```sh
"$cli" build examples/詞數統計/main.voil -o ./word-count
"$cli" pack examples/詞數統計/main.voil -o ./word-count-portable
./word-count-portable examples/詞數統計/sample.txt
```

建置機需要 rustc、系統連結器，靜態封裝另需 glibc 靜態函式庫。兩個建置命令都要求使用新的目的檔名。pack 產物可單獨放在相容的 Linux x86-64 機器執行；範例印出詞數並退出 0。一般原生程式以 main 返回值的低八位作為程序退出碼。

## 語言與版本

主線已驗收具名函式引數、控制流程、結構與資源移交、容器、Optional、UTF-8 字素操作、有限 decimal64、格式化、具型別例外與直接 await。單獨傳入 .voil 路徑仍使用舊版單檔靜態檢查入口。公開子集、限制與正反例見 [M7 施工紀錄](AI-build/M7公開語言施工.md)；專案測試及隔離交付見 [M8 驗收](AI-build/M8單語言專案.md)。

已發行的 Linux x86-64 `0.1.0-preview.1` 對應 M6 凍結基線。上面的 M7、M8 範例請使用主線建置版本。[預覽版下載與校驗碼](https://github.com/siyujellyfish/voiles/tree/releases/0.1.0-preview.1/dist) · [預覽版使用說明](release/使用說明.md)

語法與語意以[語言規格](AI-build/語言規格-v0.2.md)為準。歷史階段、設計決策與工程紀錄可從[工程文件索引](AI-build/README.md)查閱。
