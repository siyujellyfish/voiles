//! VB16-01-A：直接由真實 voiles 來源觀察結構建立的正反驗證。
//! 驗證來源解析後具名欄位先預檢，以及未覆蓋預設仍須檢查。
#[cfg(test)]
mod tests {
    use crate::struct_stage::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn vb16_具名來源順序任意而未覆蓋預設仍合法() {
        let src="struct Config{\nvar text:str = \"default\"\nconst fixed:bool = true\nvar amount:int = 1\n}\nfn main()->int{\nvar value:Config = Config(amount=2,text=\"named\")\nreturn value.amount\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn vb16_未知欄位應先拒絕而非先消耗來源的明確移動() {
        let src="struct S{\nvar text:str = \"default\"\n}\nfn main()->int{\nvar owner:str = \"source\"\nvar item:S = S(text=owner.move(),missing=1)\nreturn 0\n}\n";
        let err=inspect(src).unwrap_err();
        assert_eq!(err.kind,DiagnosticKind::Static);
        assert!(err.message.contains("UnknownField"),"{}",err.message);
    }

    #[test]
    fn vb16_重複具名欄位在實際移動運算式求值前拒絕() {
        let src="struct S{\nvar text:str = \"default\"\n}\nfn main()->int{\nvar owner:str = \"source\"\nvar item:S = S(text=owner.move(),text=\"other\")\nreturn 0\n}\n";
        let err=inspect(src).unwrap_err();
        assert_eq!(err.kind,DiagnosticKind::Static);
        assert!(err.message.contains("DuplicateField"),"{}",err.message);
    }

    #[test]
    fn vb16_未覆蓋預設的型別錯誤不得因存在建立式而略過() {
        let src="struct S{\nvar value:int = \"wrong\"\n}\nfn main()->int{\nvar item:S = S()\nreturn 0\n}\n";
        let err=inspect(src).unwrap_err();
        assert_eq!(err.kind,DiagnosticKind::Static);
        assert!(err.message.contains("型別")||err.message.contains("VB16"),"{}",err.message);
    }

    #[test]
    fn vb16_未完全初始化_var_不因其他預設存在就允許讀取() {
        let src="struct S{\nvar late:int\nvar text:str = \"default\"\n}\nfn main()->int{\nvar item:S = S()\nreturn item.late\n}\n";
        let err=inspect(src).unwrap_err();
        assert_eq!(err.kind,DiagnosticKind::Static);
        assert!(err.message.contains("初始化"),"{}",err.message);
    }

    #[test]
    fn vb16_具名欄位明確整筆移動不會被預設求值額外複製() {
        let src="struct S{\nvar owned:str = \"default\"\nconst id:int = 1\n}\nfn main()->int{\nvar owner:str = \"value\"\nvar item:S = S(owned=owner.move())\nreturn item.id\n}\n";
        assert!(inspect(src).is_ok());
    }
}
