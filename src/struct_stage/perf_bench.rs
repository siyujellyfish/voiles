//! P16.6-R04：僅在明確要求下輸出 Rust 私有靜態檢查的性能觀測。
//! 測試成功標準是來源語意通過；時間不能用作 CI 失敗條件。

#[cfg(test)]
mod p16_6_r04_私有資料流效能量測 {
    use super::inspect;
    use std::time::Instant;

    fn program(variables: usize, control_flows: usize, loop_mode: bool) -> String {
        let mut source = String::from("fn main()->int{\n");
        for idx in 0..variables {
            source.push_str(&format!("var value_{idx}:int = 0\n"));
        }
        for idx in 0..control_flows {
            let target = idx % variables;
            if loop_mode {
                source.push_str(&format!(
                    "for index_{idx} in 0..1{{\nvalue_{target} = 1\n}}\n"
                ));
            } else {
                source.push_str(&format!(
                    "if(true){{\nvalue_{target} = 1\n}}else{{\nvalue_{target} = 2\n}}\n"
                ));
            }
        }
        source.push_str("return value_0\n}\n");
        source
    }

    fn observe(variant: &str, variables: usize, branches: usize, repeats: usize) {
        let code = program(variables, branches, variant == "for");
        assert!(inspect(&code).is_ok(),
            "受限私有來源不得因效能測試而接受無效程式：{variant}/{variables}/{branches}");
        // 熱身與多次重複：數值僅作為同一 CI 主機／Debug 編譯環境的觀測基線。
        let mut timings = Vec::with_capacity(repeats);
        for _ in 0..repeats + 1 {
            let start = Instant::now();
            let accepted = inspect(std::hint::black_box(&code));
            assert!(accepted.is_ok());
            timings.push(start.elapsed().as_micros());
        }
        timings.remove(0);
        timings.sort_unstable();
        println!("P16.6-R04 benchmark kind={variant} variables={variables} branches={branches} bytes={} median_us={} samples={repeats} build=rustc-test-debug",
            code.len(), timings[repeats / 2]);
    }

    #[test]
    fn p16_6_r04_分支與迴圈負載擴展_靜態語意保持不變() {
        for &loop_mode in &[false, true] {
            let code = program(8, 4, loop_mode);
            assert!(inspect(&code).is_ok());
        }
        if std::env::var_os("VOILES_PERF_RUN").is_none() {
            return;
        }
        // 六組資料點，不設定不具證據的固定時限或跨 CI runner 倍率限制。
        for variant in ["if", "for"] {
            for (vars, branches) in [(8, 8), (32, 24), (96, 48)] {
                observe(variant, vars, branches, 5);
            }
        }
    }
}
