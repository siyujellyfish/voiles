// F00／G0：真實已驗證私有來源的八條 P16 資源橋接基準採集。
// 所有執行事件只能來自既有 trace_* 正式檢查後受限執行路徑。
// 本模組僅在 Rust 內部測試建置中存在，不變更公開 MVP-A／P17。
#[cfg(test)]
mod f00_八橋接真正資源事件基準 {
    use super::*;
    use crate::owned_str_runtime::ResourceEvent;

    fn hex(source: &[u8]) -> String {
        source.iter().map(|value| format!("{value:02x}")).collect()
    }

    fn record(key: &str, source: &str, events: &[ResourceEvent]) {
        let event_text = events.iter().map(|event| format!("{event:?}"))
            .collect::<Vec<_>>().join(",");
        println!("F00_BRIDGE_RECORD={key}\t{}\t{event_text}", hex(source.as_bytes()));
    }

    #[test]
    fn 從真正已驗證來源採集八條橋接及其成功失敗事件() {
        if std::env::var_os("VOILES_F00_BRIDGE_EXPORT").is_none() {
            return;
        }

        const 建構: &str = "struct Message{\nvar first:str = \"甲預設\"\nvar second:str = \"乙預設\"\nvar third:str = \"丙預設\"\n}\nfn main()->int{\nvar item:Message = Message(second=\"乙具名\",first=\"甲具名\")\nreturn 0\n}\n";
        let success = trace_string_struct_source(建構, "item", None).unwrap();
        record("01_建構_成功", 建構, &success.resources);
        let failed = trace_string_struct_source(建構, "item", Some("first")).unwrap();
        record("01_建構_中途失敗", 建構, &failed.resources);

        const 整筆移動: &str = "struct Message{\nvar title:str = \"預設\"\nconst id:str = \"固定🪼\"\n}\nfn main()->int{\nvar original:Message = Message(title=\"繁體中文\")\nvar transferred:Message = original.move()\nreturn 0\n}\n";
        let (_, success) = trace_persistent_struct_move_source(整筆移動, None).unwrap();
        record("02_結構移動_成功", 整筆移動, &success);
        let (_, failure) = trace_persistent_struct_move_source(整筆移動, Some("title")).unwrap();
        record("02_結構移動_建構失敗", 整筆移動, &failure);

        const 字串返回: &str = "fn create()->str{\nvar payload:str = \"繁體中文🪼\"\nreturn payload.move()\n}\nfn main()->int{\nvar received:str = create()\nreturn 0\n}\n";
        let (_, events) = trace_string_return_source(字串返回, "create", "received").unwrap();
        record("03_字串返回_成功", 字串返回, &events);

        const 欄位覆寫: &str = "struct Holder{\nvar title:str = \"舊值\"\nconst id:str = \"固定🪼\"\n}\nfn main()->int{\nvar owner:Holder = Holder()\nowner.title = \"新值🪼\"\nreturn 0\n}\n";
        let (_, events) = trace_persistent_struct_field_write_source(欄位覆寫, false).unwrap();
        record("04_欄位覆寫_成功", 欄位覆寫, &events);
        let (_, events) = trace_persistent_struct_field_write_source(欄位覆寫, true).unwrap();
        record("04_欄位覆寫_右值失敗", 欄位覆寫, &events);

        const 混合結構: &str = "struct Mixed{\nvar num:int = 3\nvar text:str = \"舊值\"\nconst flag:bool = true\n}\nfn main()->int{\nvar original:Mixed = Mixed(text=\"繁體中文🪼\",num=7)\nvar target:Mixed = original.move()\nreturn 0\n}\n";
        let (_, events) = trace_mixed_struct_source(混合結構, None).unwrap();
        record("05_混合結構_成功", 混合結構, &events);
        let (_, events) = trace_mixed_struct_source(混合結構, Some("flag")).unwrap();
        record("05_混合結構_求值失敗", 混合結構, &events);

        const 單純值: &str = "struct Plain{\nvar num:int = 3\nconst flag:bool = true\n}\nfn main()->int{\nvar original:Plain = Plain(num=9)\nvar target:Plain = original\nreturn 0\n}\n";
        let (_, events) = trace_mixed_struct_source(單純值, None).unwrap();
        record("05_純值結構_無資源", 單純值, &events);

        const 作用域: &str = "struct Holder{\nvar text:str = \"預設\"\n}\nfn main()->int{\nvar outer:Holder = Holder(text=\"外層\")\nif(true){\nvar inner:Holder = Holder(text=\"內層\")\n}\nreturn 0\n}\n";
        let events = trace_scope_struct_source(作用域, None).unwrap();
        record("06_作用域_成立分支", 作用域, &events);
        let otherwise = 作用域.replace("if(true)", "if(false)");
        let events = trace_scope_struct_source(&otherwise, None).unwrap();
        record("06_作用域_未成立分支", &otherwise, &events);

        const 結構返回: &str = "struct Bundle{\nvar title:str = \"預設\"\nconst id:str = \"固定🪼\"\n}\nfn provide()->Bundle{\nvar local:Bundle = Bundle(title=\"繁體中文\")\nreturn local.move()\n}\nfn main()->int{\nvar received:Bundle = provide()\nreturn 0\n}\n";
        let (_, events) = trace_persistent_struct_return_source(
            結構返回, "provide", "received", None).unwrap();
        record("07_結構返回_成功", 結構返回, &events);
        let (_, events) = trace_persistent_struct_return_source(
            結構返回, "provide", "received", Some("id")).unwrap();
        record("07_結構返回_建構失敗", 結構返回, &events);

        const 具名字串移動: &str = "fn main()->int{\nvar owner:str = \"繁體中文 🌊\"\nvar target:str = owner.move()\nreturn 0\n}\n";
        let events = trace_named_string_move_source(具名字串移動).unwrap();
        record("08_具名字串移動_成功", 具名字串移動, &events);
    }
}
