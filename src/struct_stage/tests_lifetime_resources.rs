#[cfg(test)]
mod p13_4_else_if_來源合流 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 已核准_else_if_中各分支_copyable_賦值都可安全合流() {
        let src = "fn main()->int{\nvar value:int = 1\nif(true){\nvalue = value + 1\n} else if(false){\nvalue = value + 2\n} else {\nvalue = value + 3\n}\nreturn value\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn else_if_其中一路徑移動資源後合流不能再使用() {
        let src = "fn main()->int{\nvar text:str = \"abc\"\nif(false){\nvar taken:str = text.move()\n} else if(true){\nvar keep:int = 1\n} else {\nvar keep:int = 2\n}\nvar again:str = text.move()\nreturn 0\n}\n";
        let error=inspect(src).expect_err("存在可達路徑已移動來源，不得使用");
        assert_eq!(error.kind,DiagnosticKind::Static);
    }

    #[test]
    fn else_if_全部分支提前回傳不應被不可達移動干擾() {
        let src = "fn main()->int{\nvar text:str = \"safe\"\nif(true){\nreturn 1\n} else if(false){\nreturn 2\n} else {\nreturn 3\n}\nreturn 4\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p15_遮蔽借用最後使用點精度 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 分支內新別名遮蔽後不再引用外層借用_可歸還原來源() {
        let src = "fn main()->int{\nconst source:int = 1\nconst alias:int = source.borrow()\nif(true){\nvar alias:int = 2\nvar copy:int = source\nreturn alias\n}\nreturn 0\n}\n";
        assert!(inspect(src).is_ok(), "遮蔽後的別名使用不得誤延長外層借用");
    }

    #[test]
    fn 分支內仍引用外層借用時不得提前解鎖來源() {
        let src = "fn main()->int{\nconst source:int = 1\nconst alias:int = source.borrow()\nif(true){\nvar copy:int = source\nreturn alias\n}\nreturn 0\n}\n";
        let error=inspect(src).expect_err("外層別名仍有效不得直接使用來源");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }
}

#[cfg(test)]
mod p13_方法純值運算式統一檢查 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 讀取_self_欄位與參數的加減可獨立回傳() {
        let src = "struct Counter{\nvar count:int = 4\nfn plus(amount:int)->int{\nreturn (self.count + amount) - 1\n}\n}\nfn main()->int{\nconst counter:Counter = Counter()\nreturn counter.plus(3)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 可寫方法以純值加減更新_self_欄位() {
        let src = "struct Counter{\nvar count:int = 4\nfn plus(amount:int)->int{\nself.count = self.count + amount\nreturn self.count\n}\n}\nfn main()->int{\nvar counter:Counter = Counter()\nreturn counter.plus(3)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 可寫方法的_self_原欄位與加減型別必須匹配() {
        let src = "struct Counter{\nvar count:int = 4\nfn wrong(amount:bool)->int{\nself.count = self.count + amount\nreturn 0\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("方法加減必須驗證 int 運算元");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("int"));
    }

    #[test]
    fn 方法不能把非_copyable_self_欄位藏進加減中() {
        let src = "struct Counter{\nvar resource:str = \"x\"\nfn bad()->int{\nreturn self.resource + 1\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("巢狀資源欄位仍不得隱含複製");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }
}

#[cfg(test)]
mod p13_方法呼叫純_copyable_一般函式 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 方法回傳可引用合法純值函式計算與_self_欄位() {
        let src = "fn add_one(value:int)->int{\nreturn value + 1\n}\nstruct Counter{\nvar amount:int = 5\nfn calculated(value:int)->int{\nreturn add_one(self.amount + value)\n}\n}\nfn main()->int{\nconst counter:Counter = Counter()\nreturn counter.calculated(3)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 方法能呼叫返回_copyable_的零引數一般函式() {
        let src = "fn base()->int{\nreturn 9\n}\nstruct Box{\nvar count:int = 0\nfn read()->int{\nreturn base()\n}\n}\nfn main()->int{\nconst instance:Box = Box()\nreturn instance.read()\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 方法內函式的資源引數不可隱含複製() {
        let src = "fn use_text(text:str)->int{\nreturn 1\n}\nstruct Box{\nvar text:str = \"資源\"\nfn bad()->int{\nreturn use_text(self.text)\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("方法不能偷複製資源欄位");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }

    #[test]
    fn 方法內一般函式新建資源可直接回傳獨立所有權() {
        let src = "fn make_text()->str{\nreturn \"資源\"\n}\nstruct Box{\nvar count:int = 0\nfn fresh()->str{\nreturn make_text()\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_方法可達分支提前回傳 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 方法分支可合法提前回傳_copyable_欄位及參數運算式() {
        let src="struct Sample{\nvar amount:int = 4\nfn value(flag:bool, delta:int)->int{\nif(flag){\nreturn self.amount + delta\n}\nreturn self.amount\n}\n}\nfn main()->int{\nconst sample:Sample = Sample()\nreturn sample.value(true, 3)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 可寫方法的分支提前回傳仍須自動推導獨占_self() {
        let src="struct Sample{\nvar amount:int = 4\nfn value(flag:bool, delta:int)->int{\nif(flag){\nself.amount = delta\nreturn self.amount\n}\nreturn self.amount\n}\n}\nfn main()->int{\nvar sample:Sample = Sample()\nreturn sample.value(true, 3)\n}\n";
        assert!(inspect(src).is_ok());
        let invalid=src.replace("var sample:Sample", "const sample:Sample");
        let error=inspect(&invalid).expect_err("可寫方法要求獨占可變 self");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("const"));
    }

    #[test]
    fn 提前回傳錯誤型別不得因最後_return_正確而漏檢() {
        let src="struct Sample{\nvar amount:int = 4\nfn bad(flag:bool)->int{\nif(flag){\nreturn true\n}\nreturn self.amount\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("提前返回型別錯誤");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("回傳型別"));
    }

    #[test]
    fn 提前回傳不得隱含複製非_copyable_self_欄位() {
        let src="struct Sample{\nvar text:str = \"資源\"\nfn bad(flag:bool)->str{\nif(flag){\nreturn self.text\n}\nreturn \"ok\"\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("資源欄位不允許隱含複製");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }
}

#[cfg(test)]
mod p13_方法位置引數明確整筆移動 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn copyable_具名整筆_move_可作方法位置引數() {
        let src="struct Counter{\nvar count:int = 1\nfn use_value(value:int)->int{\nreturn value\n}\n}\nfn main()->int{\nvar input:int = 5\nconst counter:Counter = Counter()\nreturn counter.use_value(input.move())\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 方法引數移動後來源不可再讀() {
        let src="struct Counter{\nvar count:int = 1\nfn use_value(value:int)->int{\nreturn value\n}\n}\nfn main()->int{\nvar input:int = 5\nconst counter:Counter = Counter()\nvar result:int = counter.use_value(input.move())\nreturn input\n}\n";
        let error=inspect(src).expect_err("輸入來源已明確移動");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("移動"));
    }

    #[test]
    fn 方法引數不得移動同一接收者後仍借用_self() {
        let src="struct Counter{\nvar count:int = 1\nfn compare(other:Counter)->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar counter:Counter = Counter()\nreturn counter.compare(counter.move())\n}\n";
        let error=inspect(src).expect_err("方法借用前接收者已被移動");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("移動"));
    }

    #[test]
    fn 方法引數仍不可移動已借出來源() {
        let src="struct Counter{\nvar count:int = 1\nfn use_value(value:int)->int{\nreturn value\n}\n}\nfn main()->int{\nconst source:int = 1\nconst alias:int = source.borrow()\nconst counter:Counter = Counter()\nvar result:int = counter.use_value(source.move())\nreturn alias\n}\n";
        let error=inspect(src).expect_err("有效別名期間不得移動來源");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }
}

#[cfg(test)]
mod p13_3_所有權移動來源閉包 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn str_資源經函式引數及回傳可連續明確轉交() {
        let src="fn relay(source:str)->str{\nreturn source.move()\n}\nfn main()->int{\nvar source:str = \"中文\"\nvar result:str = relay(source.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 含資源結構可整筆移動但不可隱含複製() {
        let src="struct Box{\nvar content:str = \"內容\"\n}\nfn main()->int{\nvar source:Box = Box()\nvar destination:Box = source.move()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
        let invalid=src.replace("source.move()", "source");
        let error=inspect(&invalid).expect_err("資源結構不可隱含複製");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }

    #[test]
    fn 具名欄位建立式明確轉交資源不得重複使用舊來源() {
        let src="struct Box{\nvar content:str = \"預設\"\n}\nfn main()->int{\nvar source:str = \"獨立\"\nvar destination:Box = Box(content=source.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
        let invalid=src.replace("return 0", "var again:str = source.move()\nreturn 0");
        let error=inspect(&invalid).expect_err("資源建立後原持有者失效");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("移動"));
    }

    #[test]
    fn 移動借用別名不得偷轉成所有者() {
        let src="fn main()->int{\nconst source:int = 1\nconst alias:int = source.borrow()\nvar moved:int = alias.move()\nreturn 0\n}\n";
        let error=inspect(src).expect_err("借用別名不得移動");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }

    #[test]
    fn 有效借用未歸還前不得移動原持有者() {
        let src="fn main()->int{\nconst source:int = 1\nconst alias:int = source.borrow()\nvar moved:int = source.move()\nreturn alias\n}\n";
        let error=inspect(src).expect_err("原持有者借用有效期間禁止 move");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }

    #[test]
    fn 結構單欄位移動即使資源型別也不被接受() {
        let src="struct Box{\nvar content:str = \"文字\"\n}\nfn main()->int{\nvar box:Box = Box()\nvar moved:str = box.content.move()\nreturn 0\n}\n";
        let error=inspect(src).expect_err("S21 禁止部分欄位移動");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("S21"));
    }
}

#[cfg(test)]
mod p13_非_copyable_方法參數與回傳移交 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn str_可經明確整筆_move_交給方法參數() {
        let src="struct Holder{\nvar count:int = 1\nfn consume(text:str)->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar source:str = \"資源\"\nconst holder:Holder = Holder()\nreturn holder.consume(source.move())\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 非_copyable_方法唯讀參數由具名來源預設借用() {
        let src="struct Holder{\nvar count:int = 1\nfn consume(text:str)->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar source:str = \"資源\"\nconst holder:Holder = Holder()\nreturn holder.consume(source)\n}\n";
        assert!(inspect(src).is_ok(), "函式預設借用也適用合法結構方法參數");
    }

    #[test]
    fn 方法可將取得的_str_參數再次明確轉交回傳值() {
        let src="struct Holder{\nvar count:int = 1\nfn relay(text:str)->str{\nreturn text.move()\n}\n}\nfn main()->int{\nvar source:str = \"資源\"\nconst holder:Holder = Holder()\nvar result:str = holder.relay(source.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 方法不能藉一般名稱回傳_str_參數形成隱含複製() {
        let src="struct Holder{\nvar count:int = 1\nfn invalid(text:str)->str{\nreturn text\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("資源參數不可隱含複製回傳");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }

    #[test]
    fn 方法仍禁止從_self_資源欄位明確偷移動() {
        let src="struct Holder{\nvar text:str = \"資源\"\nfn invalid()->str{\nreturn self.text.move()\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let error=inspect(src).expect_err("S21 禁止個別移動結構欄位");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("S21"));
    }

    #[test]
    fn str_方法參數可由新的字串常值直接建立獨立所有權() {
        let src="struct Holder{\nvar count:int = 1\nfn consume(text:str)->int{\nreturn self.count\n}\n}\nfn main()->int{\nconst holder:Holder = Holder()\nreturn holder.consume(\"新資源\")\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_新建資源函式位置引數 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 獨立字串常值可直接移交非_copyable_函式參數() {
        let src = "fn use_it(item:str)->int{\nreturn 1\n}\nfn main()->int{\nreturn use_it(\"新建資源\")\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 巢狀呼叫中新建字串可移交且不隱含複製既有來源() {
        let src = "fn consume(item:str)->int{\nreturn 1\n}\nfn relay(value:int)->int{\nreturn value\n}\nfn main()->int{\nreturn relay(consume(\"獨立資源\"))\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 既有字串名稱作為唯讀函式參數採預設借用() {
        let src = "fn consume(item:str)->int{\nreturn 1\n}\nfn main()->int{\nvar item:str = \"原有資源\"\nreturn consume(item)\n}\n";
        assert!(inspect(src).is_ok(), "已核准預設借用，不是 str 隱含複製");
    }

    #[test]
    fn 明確移動既有字串仍合法() {
        let src = "fn consume(item:str)->int{\nreturn 1\n}\nfn main()->int{\nvar item:str = \"原有資源\"\nreturn consume(item.move())\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p13_2_一般賦值正式來源閉包 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn int_bool_float_名稱複製與一般重新賦值均保持來源有效() {
        let src="fn main()->int{\nvar a:int = 4\nvar b:int = a\nb = a\nvar flag:bool = true\nvar another:bool = flag\nanother = flag\nvar measure:float = 1.0\nvar measure_copy:float = measure\nmeasure_copy = measure\nreturn b\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn copyable_巢狀結構可以複製給不同變數及覆寫並讀取欄位() {
        let src="struct Inner{\nvar count:int = 1\n}\nstruct Outer{\nvar inner:Inner = Inner()\n}\nfn main()->int{\nvar first:Outer = Outer()\nvar second:Outer = first\nsecond = first\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn copyable_函式參數與回傳以及方法參數可重用同一來源() {
        let src="struct Boxed{\nvar count:int = 1\nfn choose(value:int)->int{\nreturn value\n}\n}\nfn echo(value:int)->int{\nreturn value\n}\nfn main()->int{\nvar amount:int = 3\nvar receiver:Boxed = Boxed()\nvar first:int = echo(amount)\nvar second:int = receiver.choose(amount)\nreturn first + second + amount\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 已繫結_str_不能經一般宣告複製() {
        let src="fn main()->int{\nvar source:str = \"字串\"\nvar duplicate:str = source\nreturn 0\n}\n";
        let err=inspect(src).expect_err("已存在資源不能隱含複製");
        assert_eq!(err.kind,DiagnosticKind::Static);
        assert!(err.message.contains("Copyable"));
    }

    #[test]
    fn 已繫結_str_不能經一般賦值隱含複製() {
        let src="fn main()->int{\nvar source:str = \"原始\"\nvar target:str = \"目標\"\ntarget = source\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn str_獨立新建值可合法重新賦值且不偷渡隱含移動() {
        let src="fn main()->int{\nvar source:str = \"原始\"\nsource = \"新值\"\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 未初始化_copyable_值依然不能複製() {
        let src="fn main()->int{\nvar source:int\nvar duplicate:int = source\nreturn duplicate\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p13_4_分支部分結構安全合流 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 不同分支各完成不同_copyable_欄位後可安全補完() {
        let src = "struct Pair{\nvar first:int\nvar second:int\n}\nfn main()->int{\nvar pair:Pair = Pair()\nif(true){\npair.first = 1\n}else{\npair.second = 2\n}\npair.first = 3\npair.second = 4\nvar copy:Pair = pair\nreturn 0\n}\n";
        assert!(inspect(src).is_ok(), "所有可能覆寫的欄位皆 Copyable，允許聯集補完");
    }

    #[test]
    fn 差異欄位包含資源時仍需保守拒絕未證明的舊值清理() {
        let src = "struct Pair{\nvar text:str\nvar number:int\n}\nfn main()->int{\nvar pair:Pair = Pair()\nif(true){\npair.text = \"a\"\n}else{\npair.number = 1\n}\npair.text = \"b\"\npair.number = 2\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind, DiagnosticKind::Static);
    }

    #[test]
    fn 某分支移動整個結構而另一分支未移動時不能靠欄位寫入復活() {
        let src = "struct Pair{\nvar first:int\nvar second:int\n}\nfn main()->int{\nvar pair:Pair = Pair()\npair.first = 1\npair.second = 2\nif(true){\nvar taken:Pair = pair.move()\n}\npair.first = 3\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind, DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p13_6_方法獨立資源回傳 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 唯讀方法可回傳一般函式新建的獨立_str_資源() {
        let src="fn fresh()->str{\nreturn \"獨立資源\"\n}\nstruct Holder{\nvar count:int = 1\nfn acquire()->str{\nreturn fresh()\n}\n}\nfn main()->int{\nconst holder:Holder = Holder()\nvar result:str = holder.acquire()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 方法可將純_copyable_參數計算交給新建資源函式() {
        let src="fn fresh(code:int)->str{\nreturn \"獨立資源\"\n}\nstruct Holder{\nvar count:int = 1\nfn acquire(increment:int)->str{\nreturn fresh(self.count + increment)\n}\n}\nfn main()->int{\nconst holder:Holder = Holder()\nvar result:str = holder.acquire(3)\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 方法不得將已繫結_str_當成純值引數偷偷複製() {
        let src="fn passthrough(value:str)->str{\nreturn value.move()\n}\nstruct Holder{\nvar count:int = 1\nfn bad(value:str)->str{\nreturn passthrough(value)\n}\n}\nfn main()->int{\nreturn 0\n}\n";
        let err=inspect(src).expect_err("方法新建資源的引數不得隱含複製 str");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("Copyable"));
    }
}

#[cfg(test)]
mod p13_4_正式來源閉包 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 提前回傳分支的移動不污染唯一可達的後續路徑() {
        let src="fn choose(flag:bool)->int{\nvar text:str = \"原始\"\nif(flag){\nvar gone:str = text.move()\nreturn 1\n}\nvar retained:str = text.move()\nreturn 2\n}\nfn main()->int{\nreturn choose(true)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 巢狀兩側均提前回傳時不允許錯誤合流來源() {
        let src="fn choose(flag:bool)->int{\nvar text:str = \"原始\"\nif(flag){\nif(flag){\nvar gone:str = text.move()\nreturn 1\n}else{\nvar gone:str = text.move()\nreturn 2\n}\n}\nvar retained:str = text.move()\nreturn 3\n}\nfn main()->int{\nreturn choose(false)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 兩側資源移動後重新初始化可在合流後再移交() {
        let src="fn main()->int{\nvar item:str = \"原值\"\nif(true){\nvar taken:str = item.move()\nitem = \"甲\"\n}else{\nvar taken:str = item.move()\nitem = \"乙\"\n}\nvar final_item:str = item.move()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 只有一側重新初始化不可在合流後當成有效資源() {
        let src="fn main()->int{\nvar item:str = \"原值\"\nif(true){\nvar taken:str = item.move()\nitem = \"甲\"\n}else{\nvar taken:str = item.move()\n}\nvar final_item:str = item.move()\nreturn 0\n}\n";
        let err=inspect(src).expect_err("仍有可達路徑使來源已移動");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }

    #[test]
    fn 內層同名資源移動不得污染外層宣告() {
        let src="fn main()->int{\nvar item:str = \"外層\"\nif(true){\nvar item:str = \"內層\"\nvar taken:str = item.move()\n}\nvar retained:str = item.move()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 無_else_單側移動後必須拒絕合流再使用() {
        let src="fn main()->int{\nvar item:str = \"原值\"\nif(true){\nvar taken:str = item.move()\n}\nvar again:str = item.move()\nreturn 0\n}\n";
        let err=inspect(src).expect_err("單側移動必須維持來源可能無效");
        assert_eq!(err.kind,DiagnosticKind::Static);
        assert!(err.message.contains("可能") || err.message.contains("移動"));
    }

    #[test]
    fn 分支局部宣告不能跨作用域逃逸() {
        let src="fn main()->int{\nif(true){\nvar temporary:str = \"內層\"\n}\nvar escaped:str = temporary.move()\nreturn 0\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p14_1_p14_2_真實來源與衝突矩陣 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn const_來源兩個共享別名可以同時讀取且歸還後使用原來源() {
        let src="fn main()->int{\nconst source:int = 1\nconst first:int = source.borrow()\nconst second:int = source.borrow()\nvar sum:int = first + second\nreturn source\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 不同持有者可各自保有獨占借用而不互相衝突() {
        let src="fn main()->int{\nvar left:int = 1\nvar right:int = 2\nvar left_alias:int = left.borrow()\nvar right_alias:int = right.borrow()\nreturn left_alias + right_alias\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 同一持有者的兩個獨占借用不得重疊() {
        let src="fn main()->int{\nvar source:int = 1\nvar first:int = source.borrow()\nvar second:int = source.borrow()\nreturn first + second\n}\n";
        let error=inspect(src).expect_err("同源獨占借用衝突");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("HC14"));
    }

    #[test]
    fn 有效獨占借用期間不得直接讀取原來源() {
        let src="fn main()->int{\nvar source:int = 1\nvar alias:int = source.borrow()\nvar copied:int = source\nreturn alias\n}\n";
        let error=inspect(src).expect_err("借用期間不可直接讀取");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("HC14"));
    }

    #[test]
    fn const_來源不能冒充可變來源建立獨占別名() {
        let src="fn main()->int{\nconst source:int = 1\nvar alias:int = source.borrow()\nreturn alias\n}\n";
        let error=inspect(src).expect_err("借用權限不可提升");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("權限"));
    }

    #[test]
    fn 已移動資源不允許借用或憑別名建立第二所有者() {
        let moved="fn main()->int{\nvar source:str = \"原始\"\nvar taken:str = source.move()\nvar alias:str = source.borrow()\nreturn 0\n}\n";
        assert_eq!(inspect(moved).unwrap_err().kind,DiagnosticKind::Static);
        let alias_move="fn main()->int{\nvar source:str = \"原始\"\nvar alias:str = source.borrow()\nvar taken:str = alias.move()\nreturn 0\n}\n";
        assert_eq!(inspect(alias_move).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn 已存在借用別名不得在缺乏父子證明時再次借用() {
        let src="fn main()->int{\nconst source:int = 1\nconst alias:int = source.borrow()\nconst nested:int = alias.borrow()\nreturn nested\n}\n";
        let error=inspect(src).expect_err("VB14-01-A 再借用仍保守拒絕");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("再次借用"));
    }
}

#[cfg(test)]
mod p14_5_p14_6_巢狀呼叫及_self_權限 {
    use super::inspect;
    use crate::DiagnosticKind;

    const MODEL: &str="fn relay(value:int)->int{\nreturn value\n}\nstruct Item{\nvar count:int = 1\nfn read()->int{\nreturn self.count\n}\nfn set(value:int)->int{\nself.count = value\nreturn self.count\n}\n}\n";

    #[test]
    fn const_共享別名透過巢狀函式與唯讀方法可取得獨立值() {
        let src=format!("{MODEL}fn main()->int{{\nconst item:Item = Item()\nconst alias:Item = item.borrow()\nreturn relay(relay(alias.read()))\n}}\n");
        assert!(inspect(&src).is_ok());
    }

    #[test]
    fn 巢狀函式不得在共享別名仍使用時直接操作原持有者() {
        let src=format!("{MODEL}fn main()->int{{\nconst item:Item = Item()\nconst alias:Item = item.borrow()\nvar result:int = relay(item.read())\nreturn relay(alias.read())\n}}\n");
        let error=inspect(&src).expect_err("共享別名活躍時不得繞過借用使用原來源");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("HC14"));
    }

    #[test]
    fn var_獨占別名可以呼叫寫入方法並於最後使用後歸還() {
        let src=format!("{MODEL}fn main()->int{{\nvar item:Item = Item()\nvar alias:Item = item.borrow()\nvar computed:int = relay(alias.set(2))\nvar transferred:Item = item.move()\nreturn computed\n}}\n");
        assert!(inspect(&src).is_ok());
    }

    #[test]
    fn const_共享別名不能呼叫會寫入_self_的方法() {
        let src=format!("{MODEL}fn main()->int{{\nconst item:Item = Item()\nconst alias:Item = item.borrow()\nreturn alias.set(2)\n}}\n");
        let error=inspect(&src).expect_err("共享唯讀借用不可升級為獨占寫入");
        assert_eq!(error.kind,DiagnosticKind::Static);
    }

    #[test]
    fn var_借用期間不得以嵌套方法直接再次使用原持有者() {
        let src=format!("{MODEL}fn main()->int{{\nvar item:Item = Item()\nvar alias:Item = item.borrow()\nvar result:int = relay(item.set(4))\nreturn relay(alias.read())\n}}\n");
        assert_eq!(inspect(&src).unwrap_err().kind,DiagnosticKind::Static);
    }

    #[test]
    fn 非擁有別名永遠不能藉_move_成為完整資源持有者() {
        let src=format!("{MODEL}fn main()->int{{\nvar item:Item = Item()\nvar alias:Item = item.borrow()\nvar consumed:Item = alias.move()\nreturn 0\n}}\n");
        assert_eq!(inspect(&src).unwrap_err().kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p14_4_借用別名複製能力來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 已核准_copyable_借用值可建立真正獨立複本() {
        let src="fn main()->int{\nconst source:int = 9\nconst alias:int = source.borrow()\nvar copied:int = alias\nreturn copied + source\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 資源借用別名不得藉一般名稱轉為新擁有者() {
        let src="fn main()->int{\nconst source:str = \"來源\"\nconst alias:str = source.borrow()\nvar copied:str = alias\nreturn 0\n}\n";
        let error=inspect(src).expect_err("非 Copyable 借用不能複製出第二持有者");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("Copyable"));
    }
}

#[cfg(test)]
mod p15_1_父區塊後繼與內層宣告身分 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 父區塊引用外層同名變數不可延長內層借用有效期() {
        let src="fn main()->int{\nvar source:int = 1\nconst alias:int = 99\nif(true){\nvar alias:int = source.borrow()\nvar copied:int = alias\nvar after:int = source\n}\nreturn alias\n}\n";
        assert!(inspect(src).is_ok(), "外層同名引用不是內層借用最後使用點");
    }

    #[test]
    fn 父區塊真正再次引用外層借用仍應封鎖分支內原來源() {
        let src="fn main()->int{\nvar source:int = 1\nvar alias:int = source.borrow()\nif(true){\nvar another:int = source\n}\nreturn alias\n}\n";
        let error=inspect(src).expect_err("外層借用在分支後仍使用不得提前解除");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("HC14"));
    }
}

#[cfg(test)]
mod p15_父區塊續行借用來源身分 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 後續分支遮蔽同名借用不應延長舊借用有效期() {
        let src = "fn main()->int{\nconst owner:bool = true\nconst alias:bool = owner.borrow()\nif(alias){\nvar copied:bool = owner\n}else{\nvar copied:bool = false\n}\nif(true){\nconst alias:bool = false\nreturn 1\n}else{\nreturn 0\n}\nreturn 0\n}\n";
        assert!(inspect(src).is_ok(), "後續新作用域同名變數不得延長原借用");
    }

    #[test]
    fn 後續真正使用外層借用仍須封鎖來源() {
        let src = "fn main()->int{\nconst owner:bool = true\nconst alias:bool = owner.borrow()\nif(alias){\nvar copied:bool = owner\n}else{\nvar copied:bool = false\n}\nif(alias){\nreturn 1\n}else{\nreturn 0\n}\nreturn 0\n}\n";
        let error = inspect(src).expect_err("後續仍使用原借用，不可提早歸還");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }
    #[test]
    fn 內層同名遮蔽不能抹除離開區塊後的外層使用() {
        let src = "fn main()->int{\nconst owner:bool = true\nconst alias:bool = owner.borrow()\nif(true){\nif(alias){\nvar copied:bool = owner\n}else{\nvar copied:bool = false\n}\nconst alias:bool = false\nvar used:bool = alias\n}else{\nvar used:bool = false\n}\nif(alias){\nreturn 1\n}else{\nreturn 0\n}\nreturn 0\n}\n";
        let error = inspect(src).expect_err("外層借用在內層退出後仍被使用，不能過早歸還");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }

}

#[cfg(test)]
mod p15_2_借用回傳與逃逸正式來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn copyable_借用值轉成獨立回傳值仍安全合法() {
        let src = "fn copied()->int{\nconst owner:int = 1\nconst alias:int = owner.borrow()\nreturn alias\n}\nfn main()->int{\nreturn copied()\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 非_copyable_來源的借用別名不得跨函式回傳() {
        let src = "fn escape()->str{\nvar owner:str = \"資源\"\nvar alias:str = owner.borrow()\nreturn alias\n}\nfn main()->int{\nreturn 0\n}\n";
        let error = inspect(src).expect_err("借用 alias 不得建立新的資源所有者或逃逸");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("Copyable") || error.message.contains("借用"));
    }

    #[test]
    fn 借用運算式不可直接當函式回傳值() {
        let src = "fn escape()->str{\nvar owner:str = \"資源\"\nreturn owner.borrow()\n}\nfn main()->int{\nreturn 0\n}\n";
        let error = inspect(src).expect_err("直接回傳非擁有引用尚未證明安全");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("borrow") || error.message.contains("借用"));
    }

    #[test]
    fn 借用別名不可透過_move_偷換成跨作用域所有者() {
        let src = "fn escape()->str{\nvar owner:str = \"資源\"\nvar alias:str = owner.borrow()\nreturn alias.move()\n}\nfn main()->int{\nreturn 0\n}\n";
        let error = inspect(src).expect_err("借用不具備所有者轉移資格");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }

    #[test]
    fn 巢狀區塊局部資源借用不得存入外層擁有者() {
        let src = "fn main()->int{\nvar outer:str\nif(true){\nvar owner:str = \"局部\"\nvar alias:str = owner.borrow()\nouter = alias\n}else{\nvar dummy:int = 0\n}\nreturn 0\n}\n";
        let error = inspect(src).expect_err("非 Copyable 局部借用不得作為外層擁有者");
        assert_eq!(error.kind, DiagnosticKind::Static);
    }

    #[test]
    fn 活躍借用跨移動原來源必須拒絕() {
        let src = "fn main()->int{\nvar owner:str = \"資源\"\nvar alias:str = owner.borrow()\nvar transferred:str = owner.move()\nvar illegal:str = alias.move()\nreturn 0\n}\n";
        let error = inspect(src).expect_err("借用仍有效時不可將來源移出");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }

    #[test]
    fn 巢狀區塊純值回傳不應誤判為借用逃逸() {
        let src = "fn main()->int{\nvar owner:int = 1\nif(true){\nvar alias:int = owner.borrow()\nreturn alias\n}\nreturn owner\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p15_3_while_反向邊安全來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 局部借用每次迭代歸還後原來源可正常讀取() {
        let src = "fn main()->int{\nvar owner:int = 3\nwhile(false){\nvar alias:int = owner.borrow()\nvar copied:int = alias\nvar direct:int = owner\n}\nreturn owner\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 迴圈條件下次仍使用的借用不可在本次本體提前歸還() {
        let src = "fn main()->int{\nvar owner:bool = true\nvar alias:bool = owner.borrow()\nwhile(alias){\nvar direct:bool = owner\n}\nreturn 0\n}\n";
        let error = inspect(src).expect_err("迴圈下一次條件仍使用借用");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }

    #[test]
    fn 迴圈反向邊不得讓移動後的來源再次使用() {
        let src = "fn main()->int{\nvar owner:str = \"resource\"\nwhile(true){\nvar moved:str = owner.move()\n}\nreturn 0\n}\n";
        let error = inspect(src).expect_err("移動外層來源的迭代不可證明穩定");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("反向邊"));
    }

    #[test]
    fn 迴圈條件不能偷偷移動具名布林來源() {
        let src = "fn main()->int{\nvar condition:bool = true\nwhile(condition.move()){\nvar count:int = 1\n}\nreturn 0\n}\n";
        let error = inspect(src).expect_err("迴圈條件必須無消耗副作用");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("while"));
    }

    #[test]
    fn 迴圈本體的區域借用名稱不可逃到迴圈出口() {
        let src = "fn main()->int{\nvar owner:int = 1\nwhile(false){\nvar alias:int = owner.borrow()\nvar copied:int = alias\n}\nreturn alias\n}\n";
        let error = inspect(src).expect_err("迭代區域借用必須在出口失效");
        assert_eq!(error.kind, DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p15_3_for_半開區間借用來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 核准半開區間的迭代名稱是可複製的局部整數() {
        let src = "fn main()->int{\nfor i in 1..4{\nvar number:int = i\n}\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 每次迭代局部借用在最後使用後可再讀來源() {
        let src = "fn main()->int{\nfor i in 1..4{\nvar owner:int = i\nvar alias:int = owner.borrow()\nvar value:int = alias\nvar direct:int = owner\n}\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn for_反向邊不得再次讀取已移動資源() {
        let src = "fn main()->int{\nvar owner:str = \"resource\"\nfor i in 1..4{\nvar moved:str = owner.move()\n}\nreturn 0\n}\n";
        let error = inspect(src).expect_err("外層來源在每次迭代後不可變成已移動");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("反向邊"));
    }

    #[test]
    fn 迴圈外仍有效的借用必須封鎖本體直接讀取原來源() {
        let src = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nfor i in 1..4{\nvar copy:int = owner\n}\nreturn alias\n}\n";
        let error = inspect(src).expect_err("外層借用在迴圈後仍有效");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }

    #[test]
    fn 區間迭代器不應逃逸迴圈作用域() {
        let src = "fn main()->int{\nfor index in 1..4{\nvar value:int = index\n}\nreturn index\n}\n";
        let error = inspect(src).expect_err("for 迭代變數不可跨作用域逃逸");
        assert_eq!(error.kind, DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p15_5_懸空與合法重新借用來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 前一筆借用最後使用後可重新借用同一原持有者() {
        let src = "fn main()->int{\nvar owner:int = 1\nvar first:int = owner.borrow()\nvar copy:int = first\nvar direct:int = owner\nvar second:int = owner.borrow()\nvar another:int = second\nreturn owner\n}\n";
        assert!(inspect(src).is_ok(), "兩次不重疊的直接原持有者借用必須合法");
    }

    #[test]
    fn 第二筆借用不可與仍有未來使用的獨占借用重疊() {
        let src = "fn main()->int{\nvar owner:int = 1\nvar first:int = owner.borrow()\nvar second:int = owner.borrow()\nreturn first\n}\n";
        let err = inspect(src).expect_err("原來源仍有有效獨占別名");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("借用"));
    }

    #[test]
    fn 局部借用不能逃逸作用域成為懸空名稱() {
        let src = "fn main()->int{\nvar owner:int = 1\nif (true){\nvar alias:int = owner.borrow()\nvar copy:int = alias\n}\nreturn alias\n}\n";
        assert_eq!(inspect(src).unwrap_err().kind, DiagnosticKind::Static);
    }

    #[test]
    fn 未來仍會使用的借用不得允許原持有者先移動() {
        let src = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nvar moved:int = owner.move()\nreturn alias\n}\n";
        let err = inspect(src).expect_err("原持有者不得早於活躍別名移動");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("借用"));
    }

    #[test]
    fn 借用別名再次借用仍保持_vb14_階段性拒絕() {
        let src = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nvar invalid:int = alias.borrow()\nreturn invalid\n}\n";
        let err = inspect(src).expect_err("不推定父子借用及重新借用的有效期");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("再") || err.message.contains("借用"));
    }

    #[test]
    fn 巢狀區塊的借用歸還不應污染外層再次借用() {
        let src = "fn main()->int{\nvar owner:int = 1\nif (true){\nvar scoped:int = owner.borrow()\nvar value:int = scoped\n}\nvar second:int = owner.borrow()\nvar copied:int = second\nreturn owner\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p15_4_欄位索引與容器借用界線 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 可複製的結構欄位直接讀取仍合法且非借用() {
        let src = "struct Sample{\nvar count:int = 2\n}\nfn main()->int{\nvar owner:Sample = Sample()\nvar copied:int = owner.count\nreturn copied\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 直接借用結構欄位不應偷建未驗證的子來源別名() {
        let src = "struct Sample{\nvar count:int = 2\n}\nfn main()->int{\nvar owner:Sample = Sample()\nvar alias:int = owner.count.borrow()\nreturn alias\n}\n";
        let err = inspect(src).expect_err("欄位借用尚無有效期證明");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("P15.4"));
    }

    #[test]
    fn 非_copyable_字串欄位不得借用逃逸() {
        let src = "struct Sample{\nvar title:str = \"文字\"\n}\nfn main()->int{\nvar owner:Sample = Sample()\nvar alias:str = owner.title.borrow()\nreturn 0\n}\n";
        let err = inspect(src).expect_err("非 Copyable 子資源不得建立未證明別名");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("P15.4"));
    }

    #[test]
    fn 索引元素不得在沒有別名存活證明前被當作借用來源() {
        let src = "fn main()->int{\nvar owner:str = \"甲乙\"\nvar alias:str = owner[0].borrow()\nreturn 0\n}\n";
        let err = inspect(src).expect_err("索引來源尚未建立經驗證的借用文法");
        assert_eq!(err.kind, DiagnosticKind::Syntax);
        assert!(err.message.contains("P15.4"));
    }

    #[test]
    fn 已核准的完整具名借用仍保持合法() {
        let src = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nvar copied:int = alias\nreturn owner\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p15_6_最後使用點與_self_退出邊界來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 一般借用不必等函式區塊出口即可歸還() {
        let src = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nvar snapshot:int = alias\nvar direct:int = owner\nreturn direct\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn self_借用於唯讀方法返回後才歸還並允許接收者整筆移動() {
        let src = "struct Holder{\nvar count:int = 1\nfn view()->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar owner:Holder = Holder()\nvar observed:int = owner.view()\nvar moved:Holder = owner.move()\nreturn observed\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn self_方法不可在仍活躍的原來源借用期間偷拿新接收者() {
        let src = "struct Holder{\nvar count:int = 1\nfn view()->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar owner:Holder = Holder()\nvar alias:Holder = owner.borrow()\nvar invalid:int = owner.view()\nreturn alias.view()\n}\n";
        let err=inspect(src).expect_err("別名仍有效時不得直接方法存取原持有者");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("借用"));
    }

    #[test]
    fn 函式接受已核准_copyable_借用值的獨立複製不造成參數逃逸() {
        let src = "fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nvar owner:int = 3\nvar alias:int = owner.borrow()\nvar copy:int = identity(alias)\nvar direct:int = owner\nreturn copy + direct\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 尚未建立_await_執行期證明前不得放行有效別名跨越語法邊界() {
        let src = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nawait alias\nreturn alias\n}\n";
        assert!(inspect(src).is_err(), "VB15-01-A 的跨 await 合法來源尚待完整語法與存活證明");
    }

    #[test]
    fn 尚未建立_例外_歸還證明前不得將錯誤邊界當成合法來源() {
        let src = "fn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\ntry{\nreturn alias\n}\nreturn 0\n}\n";
        assert!(inspect(src).is_err(), "未證明例外歸還語意不得接受");
    }
}
#[cfg(test)]
mod p15_3_單次區間與函式借用閉包 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 單次半開區間移動後可合法重新初始化() {
        let src = "fn main()->int{\nvar owner:int = 1\nfor i in 0..1{\nvar transferred:int = owner.move()\n}\nowner = 2\nreturn owner\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 單次半開區間已移動來源不可被後繼使用() {
        let src = "fn main()->int{\nvar owner:int = 1\nfor i in 0..1{\nvar transferred:int = owner.move()\n}\nreturn owner\n}\n";
        let error = inspect(src).expect_err("單次迭代後仍須保留移動狀態");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("移動"));
    }

    #[test]
    fn 多次區間仍須禁止未經證明的外層移動反向邊() {
        let src = "fn main()->int{\nvar owner:int = 1\nfor i in 0..2{\nvar transferred:int = owner.move()\n}\nreturn 0\n}\n";
        let error = inspect(src).expect_err("兩次以上迭代必須有反向邊安全證明");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("反向邊"));
    }

    #[test]
    fn 借用別名經函式呼叫最後使用後原來源恢復可讀() {
        let src = "fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nvar copied:int = identity(alias)\nreturn owner\n}\n";
        assert!(inspect(src).is_ok(), "函式呼叫引數必須納入最後使用點");
    }

    #[test]
    fn 呼叫之前借用尚待使用不可提前存取原來源() {
        let src = "fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nvar invalid:int = owner\nreturn identity(alias)\n}\n";
        let error = inspect(src).expect_err("函式呼叫仍將使用別名，借用不得提前歸還");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }
}

#[cfg(test)]
mod p15_3_空半開區間借用來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 空區間不得把未執行的移動當成已移動來源() {
        let src = "fn main()->int{\nvar owner:str = \"保持\"\nfor i in 2..2{\nvar moved:str = owner.move()\n}\nvar result:str = owner.move()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok(), "空半開區間不能消耗外層資源");
    }

    #[test]
    fn 相同數值但不同合法進位也應視為空區間() {
        let src = "fn main()->int{\nvar owner:int = 1\nfor i in 0x2..2{\nvar moved:int = owner.move()\n}\nreturn owner\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 非空多次區間的移動仍須拒絕不穩定反向邊() {
        let src = "fn main()->int{\nvar owner:str = \"不可重複移動\"\nfor i in 2..4{\nvar moved:str = owner.move()\n}\nreturn 0\n}\n";
        let error=inspect(src).expect_err("多次區間移動不得被空區間優化放行");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("反向邊"));
    }
}
#[cfg(test)]
mod p15_3_break_continue_轉移路徑來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 迭代區域別名可於_continue_前合法結束() {
        let src="fn main()->int{\nvar owner:int = 1\nfor i in 1..4{\nvar alias:int = owner.borrow()\nvar copy:int = alias\ncontinue\n}\nreturn owner\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn while_分支內_break_不能被當作函式_return() {
        let src="fn main()->int{\nvar value:int = 1\nwhile(true){\nif(true){\nbreak\n}\nvar untouched:int = value\n}\nreturn value\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 巢狀_for_的_break_只離開最近一層迴圈() {
        let src="fn main()->int{\nfor i in 1..4{\nfor j in 1..3{\nbreak\n}\ncontinue\n}\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 移動外層資源後_break_不得把退出狀態假裝成入口() {
        let src="fn main()->int{\nvar owner:str = \"text\"\nfor i in 1..4{\nvar moved:str = owner.move()\nbreak\n}\nreturn 0\n}\n";
        let error=inspect(src).expect_err("break 外層資源狀態不能變更");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("break") || error.message.contains("不穩定"));
    }

    #[test]
    fn 移動外層資源後_continue_不可再進下一輪() {
        let src="fn main()->int{\nvar owner:str = \"text\"\nwhile(true){\nvar moved:str = owner.move()\ncontinue\n}\nreturn 0\n}\n";
        let error=inspect(src).expect_err("continue 的反向邊不得保留已移動值");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("continue") || error.message.contains("不穩定"));
    }

    #[test]
    fn 非迴圈區塊的_break_與_continue_都必須拒絕() {
        for keyword in ["break","continue"] {
            let src=format!("fn main()->int{{\n{}\nreturn 0\n}}\n",keyword);
            let error=inspect(&src).expect_err("只有迴圈中才能控制迭代");
            assert_eq!(error.kind,DiagnosticKind::Static);
        }
    }

    #[test]
    fn 函式內迴圈跨迭代讀取有效借用值仍合法() {
        let src="fn identity(value:int)->int{\nreturn value\n}\nfn main()->int{\nvar owner:int = 1\nvar alias:int = owner.borrow()\nfor i in 1..4{\nvar result:int = identity(alias)\ncontinue\n}\nreturn owner\n}\n";
        assert!(inspect(src).is_ok());
    }
}


#[cfg(test)]
mod p15_3_迴圈轉移不等於函式返回 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 單次迴圈_break_後仍須檢查移動來源的後續使用() {
        let src = "fn main()->int{\nvar owner:str = \"資源\"\nfor i in 0..1{\nbreak\n}\nvar moved:str = owner.move()\nvar duplicate:str = owner.move()\nreturn 0\n}\n";
        let error=inspect(src).expect_err("break 不是函式 return，必須分析後續違規移動");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("移動"));
    }

    #[test]
    fn 單次迴圈_if_的_break_與_continue_均不能跳過後繼() {
        let src = "fn main()->int{\nvar owner:int = 1\nfor i in 0..1{\nif (true){\nbreak\n}else{\ncontinue\n}\n}\nvar moved:int = owner.move()\nreturn owner\n}\n";
        let error=inspect(src).expect_err("跳出迴圈後仍須檢查已移動來源");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("移動"));
    }

    #[test]
    fn 真正的_函式_return_仍可在單次迴圈內成立() {
        let src = "fn main()->int{\nfor i in 0..1{\nreturn 1\n}\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }
}

#[cfg(test)]
mod p15_6_函式參數與方法接收者歸還來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 函式參數建立的合法借用最後使用後能重新存取參數() {
        let src = "fn observe(value:int)->int{\nconst alias:int = value.borrow()\nvar copy:int = alias\nreturn value\n}\nfn main()->int{\nreturn observe(1)\n}\n";
        assert!(inspect(src).is_ok(), "參數內一般借用於最後使用點歸還");
    }

    #[test]
    fn 函式參數別名仍待使用不得提前直接複製參數() {
        let src = "fn observe(value:int)->int{\nconst alias:int = value.borrow()\nvar invalid:int = value\nreturn alias\n}\nfn main()->int{\nreturn observe(1)\n}\n";
        let error = inspect(src).expect_err("借用有效期不可因函式參數為 Copyable 而提早結束");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }

    #[test]
    fn 共享別名方法退出且別名最後使用後原來源可複製() {
        let src = "struct Holder{\nconst number:int = 1\nfn view()->int{\nreturn self.number\n}\n}\nfn main()->int{\nconst owner:Holder = Holder()\nconst alias:Holder = owner.borrow()\nvar result:int = alias.view()\nvar independent:Holder = owner\nreturn result\n}\n";
        assert!(inspect(src).is_ok(), "方法 self 只在呼叫期間，別名於最後使用點解除");
    }

    #[test]
    fn 方法別名尚待使用時原持有者不得提前複製() {
        let src = "struct Holder{\nconst number:int = 1\nfn view()->int{\nreturn self.number\n}\n}\nfn main()->int{\nconst owner:Holder = Holder()\nconst alias:Holder = owner.borrow()\nvar invalid:Holder = owner\nreturn alias.view()\n}\n";
        let error = inspect(src).expect_err("仍有效的別名封鎖來源，不能提前結束 self 關聯借用");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("借用"));
    }
}

#[cfg(test)]
mod p15_3_單次迴圈混合跳轉安全合流 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn break_與初始化分支混合不能假裝每條出口都已初始化() {
        let source = "fn main()->int{\nvar result:str\nfor i in 0..1{\nif(true){\nbreak\n}else{\nresult = \"已建立\"\n}\n}\nvar moved:str = result.move()\nreturn 0\n}\n";
        let error = inspect(source).expect_err("break 路徑仍是尚未初始化，不能沿用另一分支的狀態");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("混合迴圈跳轉"));
    }

    #[test]
    fn continue_與資源移動分支混合不能捏造唯一出口狀態() {
        let source = "fn main()->int{\nvar owner:str = \"資源\"\nfor i in 0..1{\nif(true){\ncontinue\n}else{\nvar consumed:str = owner.move()\n}\n}\nvar next:str = owner.move()\nreturn 0\n}\n";
        let error = inspect(source).expect_err("單次 continue 路徑與資源移交路徑不能直接選其中一條");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("混合迴圈跳轉"));
    }

    #[test]
    fn break_和正常分支都未改外層資源時仍安全() {
        let source = "fn main()->int{\nvar owner:str = \"資源\"\nfor i in 0..1{\nif(true){\nbreak\n}else{\nvar local:int = 1\n}\n}\nvar moved:str = owner.move()\nreturn 0\n}\n";
        assert!(inspect(source).is_ok(), "兩條路徑外層狀態一致，單次迴圈可以退出");
    }

    #[test]
    fn break_之後另一分支的後續寫入不能復活未初始化資源() {
        let source = "fn main()->int{\nvar result:str\nfor i in 0..1{\nif(true){\nbreak\n}else{\nvar local:int = 1\n}\nresult = \"晚到的初始化\"\n}\nvar moved:str = result.move()\nreturn 0\n}\n";
        let error = inspect(source).expect_err("跳轉路徑沒有執行 if 後面的初始化");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("混合迴圈跳轉"));
    }

    #[test]
    fn 內層迴圈的_break_不會錯當外層單次迴圈出口() {
        let source = "fn main()->int{\nvar owner:str = \"資源\"\nfor i in 0..1{\nfor j in 0..1{\nbreak\n}\nvar moved:str = owner.move()\n}\nreturn 0\n}\n";
        assert!(inspect(source).is_ok(), "內層 break 不應否決外層已證明單次的合法移動");
    }
}

#[cfg(test)]
mod p16_真正_voil_來源至_utf8_緩衝區清理 {
    use super::trace_string_struct_source;
    use crate::struct_initialization::BuildStatus;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::DiagnosticKind;

    const SOURCE: &str = "struct Message{\nvar first:str = \"甲預設\"\nvar second:str = \"乙預設\"\nvar third:str = \"丙預設\"\n}\nfn main()->int{\nvar item:Message = Message(second=\"乙具名\",first=\"甲具名\")\nreturn 0\n}\n";
    fn created(events: &[ResourceEvent]) -> Vec<u64> {
        events.iter().filter_map(|event| match event {
            ResourceEvent::Created(id) => Some(*id), _ => None
        }).collect()
    }
    fn released(events: &[ResourceEvent]) -> Vec<u64> {
        events.iter().filter_map(|event| match event {
            ResourceEvent::Released(id) => Some(*id), _ => None
        }).collect()
    }

    #[test]
    fn 真實來源先依具名順序再依預設配置且反向釋放() {
        let trace = trace_string_struct_source(SOURCE, "item", None).unwrap();
        assert_eq!(trace.status, BuildStatus::Complete);
        assert_eq!(trace.evaluated.iter().map(|step| step.field.as_str())
            .collect::<Vec<_>>(), vec!["second", "first", "third"]);
        assert_eq!(trace.lifetime_values, vec![
            ("second".into(), "乙具名".into()),
            ("first".into(), "甲具名".into()),
            ("third".into(), "丙預設".into()),
        ]);
        assert_eq!(created(&trace.resources), vec![1,2,3]);
        assert_eq!(released(&trace.resources), vec![3,2,1]);
    }

    #[test]
    fn 真實來源每個已成功建立的失敗切點恰好反向清理() {
        for (failed, expected) in [
            ("second", vec![1]), ("first", vec![2,1]), ("third", vec![3,2,1])
        ] {
            let trace = trace_string_struct_source(SOURCE, "item", Some(failed)).unwrap();
            assert_eq!(trace.status, BuildStatus::EvaluationFailed(failed.into()));
            let mut reversed = created(&trace.resources);
            reversed.reverse();
            assert_eq!(released(&trace.resources), expected);
            assert_eq!(released(&trace.resources), reversed);
        }
    }

    #[test]
    fn 真實來源不可把非字串欄位當成已驗證的字串資源() {
        let source = "struct Mixed{\nvar label:str = \"中文\"\nvar count:int = 1\n}\nfn main()->int{\nvar item:Mixed = Mixed()\nreturn 0\n}\n";
        let error = trace_string_struct_source(source, "item", None).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("str 欄位"));
    }

    #[test]
    fn 真實來源不能把具有轉義的原文字節當成實際字串內容() {
        let source = SOURCE.replace("乙具名", "乙\\n具名");
        let error = trace_string_struct_source(&source, "item", None).unwrap_err();
        assert!(error.message.contains("轉義") || error.kind == DiagnosticKind::Lexical
            || error.kind == DiagnosticKind::Syntax);
    }

    #[test]
    fn 真實來源不能推定分支或多語句之建立式一定執行() {
        let source = SOURCE.replace("return 0", "var another:int = 1\nreturn 0");
        let error = trace_string_struct_source(&source, "item", None).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("單次宣告"));
    }

    #[test]
    fn 真實來源不存在的失敗切點不得偽稱已完成注入() {
        let error = trace_string_struct_source(SOURCE, "item", Some("missing")).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("失敗注入"));
    }

    #[test]
    fn 真實來源仍遵守靜態借用與型別拒絕優先於資源執行() {
        let source = SOURCE.replace("first=\"甲具名\"", "first=1");
        let error = trace_string_struct_source(&source, "item", None).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("型別"));
    }
}

#[cfg(test)]
mod p16_結構來源失敗注入實際求值步驟 {
    use super::trace_string_struct_source;
    use crate::struct_initialization::BuildStatus;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::DiagnosticKind;

    const PARTIAL: &str = "struct Message{\nvar optional:str\nconst id:str = \"固定\"\n}\nfn main()->int{\nvar item:Message = Message()\nreturn 0\n}\n";

    #[test]
    fn 沒有預設也沒有提供的_var_不可當成失敗注入已執行() {
        let trace = trace_string_struct_source(PARTIAL, "item", None).unwrap();
        assert_eq!(trace.status, BuildStatus::PartiallyInitialized(
            vec!["optional".to_owned()]));
        assert_eq!(trace.resources, vec![
            ResourceEvent::Created(1), ResourceEvent::Released(1)]);
        let error = trace_string_struct_source(PARTIAL, "item", Some("optional"))
            .expect_err("var optional 沒有求值步驟，不可虛報注入成功");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("失敗注入"));
    }

    #[test]
    fn 已執行的預設欄位仍可真實失敗並釋放唯一資源() {
        let trace = trace_string_struct_source(PARTIAL, "item", Some("id")).unwrap();
        assert_eq!(trace.status, BuildStatus::EvaluationFailed("id".into()));
        assert_eq!(trace.resources, vec![
            ResourceEvent::Created(1), ResourceEvent::Released(1)]);
    }

    #[test]
    fn 具名提供原本未初始化的_var_是真正求值步驟() {
        let source = PARTIAL.replace("Message()",
            "Message(optional=\"中文資源\")");
        let trace = trace_string_struct_source(&source, "item", Some("optional")).unwrap();
        assert_eq!(trace.status,
            BuildStatus::EvaluationFailed("optional".into()));
        assert_eq!(trace.resources, vec![
            ResourceEvent::Created(1), ResourceEvent::Released(1)]);
    }
}

#[cfg(test)]
mod p16_真正_voil_函式返回資源交接 {
    use super::trace_string_return_source;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::DiagnosticKind;

    const SOURCE: &str = "fn provide()->str{\nvar payload:str = \"水母🪼\"\nreturn payload.move()\n}\nfn main()->int{\nvar received:str = provide()\nreturn 0\n}\n";

    #[test]
    fn 真實來源具名整筆移動返回後只由呼叫端清理一次() {
        let (value, events) = trace_string_return_source(SOURCE, "provide", "received").unwrap();
        assert_eq!(value, "水母🪼");
        assert_eq!(events, vec![
            ResourceEvent::Created(1),
            ResourceEvent::Returned(1),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn 真實來源獨立字串常值返回不造成第二個所有者() {
        let source = "fn provide()->str{\nreturn \"繁體中文\"\n}\nfn main()->int{\nvar received:str = provide()\nreturn 0\n}\n";
        let (value, events) = trace_string_return_source(source, "provide", "received").unwrap();
        assert_eq!(value, "繁體中文");
        assert_eq!(events, vec![
            ResourceEvent::Created(1),
            ResourceEvent::Returned(1),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn 真實來源直接借用逃逸不得進入資源交接模型() {
        let source = "fn provide()->str{\nvar payload:str = \"資源\"\nreturn payload.borrow()\n}\nfn main()->int{\nvar received:str = provide()\nreturn 0\n}\n";
        let error = trace_string_return_source(source, "provide", "received").unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("borrow") || error.message.contains("逃逸"),
            "必須明確拒絕借用逃逸而非依賴特定中文同義詞：{}", error.message);
    }

    #[test]
    fn 真實來源不可將局部字串隱含複製成返回資源() {
        let source = SOURCE.replace("payload.move()", "payload");
        let error = trace_string_return_source(&source, "provide", "received").unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("複製"));
    }

    #[test]
    fn 真實來源其他控制流程不可假定被呼叫端只執行一次() {
        let source = SOURCE.replace("return payload.move()",
            "if(true){\nreturn payload.move()\n}\nreturn \"備用\"");
        let error = trace_string_return_source(&source, "provide", "received").unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("被呼叫端"));
    }

    #[test]
    fn 真實來源其他主程式操作不可被誤認為已完成執行() {
        let source = SOURCE.replace("return 0", "var other:int = 1\nreturn 0");
        let error = trace_string_return_source(&source, "provide", "received").unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("呼叫端"));
    }

    #[test]
    fn 真實來源含轉義字串須待正確解碼器才可接資源() {
        let source = SOURCE.replace("水母🪼", "水母\\n🪼");
        let error = trace_string_return_source(&source, "provide", "received").unwrap_err();
        assert!(error.message.contains("被呼叫端")
            || error.kind == DiagnosticKind::Lexical || error.kind == DiagnosticKind::Syntax);
    }
}

#[cfg(test)]
mod p16_真正_voil_具名_str_移動資源事件 {
    use super::trace_named_string_move_source;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::DiagnosticKind;

    const VALID: &str = "fn main()->int{\nvar owner:str = \"繁體中文🪼\"\nvar destination:str = owner.move()\nreturn 0\n}\n";

    #[test]
    fn 真的_voil_字串整筆移動只建立和清理一次() {
        assert_eq!(trace_named_string_move_source(VALID).unwrap(), vec![
            ResourceEvent::Created(1),
            ResourceEvent::Moved(1),
            ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn 同一來源第二次移動在建立執行期資源前已被靜態拒絕() {
        let source = VALID.replace("return 0",
            "var second:str = owner.move()\nreturn 0");
        let error = trace_named_string_move_source(&source).expect_err("不得二次移動");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("移動") || error.message.contains("已"));
    }

    #[test]
    fn str_不可省略_move_形成第二所有者() {
        let source = VALID.replace("owner.move()", "owner");
        let error = trace_named_string_move_source(&source)
            .expect_err("非 Copyable 原來源不能隱含複製");
        assert_eq!(error.kind, DiagnosticKind::Static);
    }

    #[test]
    fn 轉義常值尚無精確字串求值器故保守拒絕() {
        let source = VALID.replace("繁體中文🪼", "繁體\\n中文");
        let error = trace_named_string_move_source(&source).expect_err("轉義須拒絕");
        assert!(error.message.contains("轉義")
            || matches!(error.kind, DiagnosticKind::Lexical | DiagnosticKind::Syntax));
    }

    #[test]
    fn 未接通的額外來源語句不能冒充已有一般執行器() {
        let source = VALID.replace("return 0", "var count:int = 1\nreturn 0");
        let error = trace_named_string_move_source(&source)
            .expect_err("多語句執行期尚未證明");
        assert_eq!(error.kind, DiagnosticKind::Static);
        assert!(error.message.contains("連續建立"));
    }
}

#[cfg(test)]
mod p15_6_非_copyable_函式預設借用來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    #[test]
    fn 具名字串唯讀借用_函式退出後可整筆移動() {
        let src = "fn see(value:str)->int{\nreturn 1\n}\nfn main()->int{\nvar owner:str = \"水母🪼\"\nvar seen:int = see(owner)\nvar target:str = owner.move()\nreturn seen\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 會消耗參數的函式仍要求呼叫端明確移動() {
        let src = "fn take(value:str)->str{\nreturn value.move()\n}\nfn main()->int{\nvar owner:str = \"資源\"\nvar invalid:str = take(owner)\nreturn 0\n}\n";
        let err=inspect(src).expect_err("被呼叫端消耗參數時不能將借用誤當所有者");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("P15.6"));
    }

    #[test]
    fn 函式預設借用參數內不得再次移交或建立不安全別名() {
        let src = "fn see(value:str)->int{\nvar alias:str = value.borrow()\nreturn 1\n}\nfn main()->int{\nvar owner:str = \"資源\"\nreturn see(owner)\n}\n";
        let err=inspect(src).expect_err("借用函式參數不持有原資源，無法保證別名再借用");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("再") || err.message.contains("借用"));
    }

    #[test]
    fn 同一函式呼叫中可寫字串的雙重獨占借用拒絕() {
        let src = "fn both(a:str,b:str)->int{\nreturn 1\n}\nfn main()->int{\nvar owner:str = \"資源\"\nreturn both(owner,owner)\n}\n";
        let err=inspect(src).expect_err("可變來源不得在同一次呼叫中有兩份獨占借用");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("HC14"));
    }

    #[test]
    fn 同一函式呼叫中唯讀常數可以共享兩個借用() {
        let src = "fn both(a:str,b:str)->int{\nreturn 1\n}\nfn main()->int{\nconst owner:str = \"資源\"\nreturn both(owner,owner)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 同一呼叫先借用後明確移動同一來源須拒絕() {
        let src = "fn mixed(a:str,b:str)->int{\nreturn 1\n}\nfn main()->int{\nvar owner:str = \"資源\"\nreturn mixed(owner,owner.move())\n}\n";
        let err=inspect(src).expect_err("呼叫中的借用尚未歸還，不能移動原來源");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("P15.6"));
    }

    #[test]
    fn 已存在獨占別名時不得再次以函式預設借用原來源() {
        let src = "struct Boxed{\nvar text:str = \"資源\"\nvar count:int = 1\n}\nfn see(value:Boxed)->int{\nreturn value.count\n}\nfn main()->int{\nvar owner:Boxed = Boxed()\nvar alias:Boxed = owner.borrow()\nvar result:int = see(owner)\nreturn alias.count\n}\n";
        let err=inspect(src).expect_err("別名仍會被使用，函式不得取得新的獨占借用");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("HC14"));
    }

    #[test]
    fn 借用參數不能以一般名稱回傳為第二個_str_所有者() {
        let src = "fn leak(value:str)->str{\nreturn value\n}\nfn main()->int{\nvar owner:str = \"資源\"\nvar result:str = leak(owner)\nreturn 0\n}\n";
        let err=inspect(src).expect_err("borrowed str 不可由回傳變成擁有值");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("Copyable"));
    }
}

#[cfg(test)]
mod p15_6_方法非_copyable_參數借用退出安全來源 {
    use super::inspect;
    use crate::DiagnosticKind;

    const HOLDER: &str = "struct Holder{\nvar count:int = 1\nfn view(text:str)->int{\nreturn self.count\n}\n}\n";

    #[test]
    fn 唯讀方法接受具名資源借用且退出後仍可移動() {
        let src=format!("{HOLDER}fn main()->int{{\nvar resource:str = \"資源🪼\"\nconst owner:Holder = Holder()\nvar value:int = owner.view(resource)\nvar taken:str = resource.move()\nreturn value\n}}\n");
        assert!(inspect(&src).is_ok());
    }

    #[test]
    fn 同次方法呼叫兩個獨占借用同一來源拒絕() {
        let src="struct Holder{\nvar count:int = 1\nfn both(a:str,b:str)->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar resource:str = \"資源\"\nconst owner:Holder = Holder()\nreturn owner.both(resource,resource)\n}\n";
        let err=inspect(src).expect_err("方法參數不得重複獨占借用");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("HC14"));
    }

    #[test]
    fn 同次方法呼叫兩個共享借用可並存() {
        let src="struct Holder{\nvar count:int = 1\nfn both(a:str,b:str)->int{\nreturn self.count\n}\n}\nfn main()->int{\nconst resource:str = \"資源\"\nconst owner:Holder = Holder()\nreturn owner.both(resource,resource)\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 方法借用仍有效時後面參數不能移動原來源() {
        let src="struct Holder{\nvar count:int = 1\nfn both(a:str,b:str)->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar resource:str = \"資源\"\nconst owner:Holder = Holder()\nreturn owner.both(resource,resource.move())\n}\n";
        let err=inspect(src).expect_err("借用參數的來源在方法呼叫期間不得再被移交");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("P15.6"));
    }

    #[test]
    fn 方法自身持有者不可當成同次參數再獨占借用() {
        let src="struct Holder{\nvar text:str = \"資源\"\nvar count:int = 1\nfn view(other:Holder)->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar owner:Holder = Holder()\nreturn owner.view(owner)\n}\n";
        let err=inspect(src).expect_err("同一所有者同時當作 self 及其參數未完成安全證明");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("self"));
    }

    #[test]
    fn 已借出的結構原來源不得進行方法預設借用參數交接() {
        let src="struct Resource{\nvar text:str = \"資源\"\nvar count:int = 1\n}\nstruct Holder{\nvar count:int = 1\nfn view(other:Resource)->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar owner:Resource = Resource()\nvar alias:Resource = owner.borrow()\nconst receiver:Holder = Holder()\nvar result:int = receiver.view(owner)\nreturn alias.count\n}\n";
        let err=inspect(src).expect_err("方法參數不得再借用活躍獨占來源");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("HC14"));
    }

    #[test]
    fn 消耗資源的方法參數仍需明確_move_且舊正例不回退() {
        let src="struct Holder{\nvar count:int = 1\nfn relay(text:str)->str{\nreturn text.move()\n}\n}\nfn main()->int{\nvar source:str = \"資源\"\nconst receiver:Holder = Holder()\nvar taken:str = receiver.relay(source.move())\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
        let illegal=src.replace("relay(source.move())","relay(source)");
        let err=inspect(&illegal).expect_err("消耗參數不是預設非擁有借用");
        assert_eq!(err.kind, DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p15_3_函式預設借用跨分支迴圈交叉驗證 {
    use super::inspect;
    use crate::DiagnosticKind;

    const SEE: &str = "fn see(value:str)->int{\nreturn 1\n}\n";

    #[test]
    fn 雙分支各自借用非_copyable_來源後可於合流移動() {
        let src=format!("{SEE}fn main()->int{{\nvar owner:str = \"資源\"\nif(true){{\nvar a:int = see(owner)\n}}else{{\nvar b:int = see(owner)\n}}\nvar moved:str = owner.move()\nreturn 0\n}}\n");
        assert!(inspect(&src).is_ok());
    }

    #[test]
    fn 巢狀條件中的函式借用退出後仍可再次借用同來源() {
        let src=format!("{SEE}fn main()->int{{\nconst owner:str = \"資源\"\nif(true){{\nif(true){{\nvar a:int = see(owner)\n}}\nvar b:int = see(owner)\n}}\nreturn see(owner)\n}}\n");
        assert!(inspect(&src).is_ok());
    }

    #[test]
    fn 確定單次_for_函式借用與_continue_不錯把來源移走() {
        let src=format!("{SEE}fn main()->int{{\nvar owner:str = \"資源\"\nfor i in 0..1{{\nvar observed:int = see(owner)\ncontinue\n}}\nvar moved:str = owner.move()\nreturn 0\n}}\n");
        assert!(inspect(&src).is_ok());
    }

    #[test]
    fn 多次_for_函式借用穩定返回且最後可移動() {
        let src=format!("{SEE}fn main()->int{{\nvar owner:str = \"資源\"\nfor i in 1..4{{\nvar observed:int = see(owner)\n}}\nvar moved:str = owner.move()\nreturn 0\n}}\n");
        assert!(inspect(&src).is_ok());
    }

    #[test]
    fn while_內方法參數借用後_break_只退出目前迴圈() {
        let src="struct Holder{\nvar count:int = 1\nfn see(value:str)->int{\nreturn self.count\n}\n}\nfn main()->int{\nvar resource:str = \"資源\"\nconst receiver:Holder = Holder()\nwhile(true){\nvar observed:int = receiver.see(resource)\nbreak\n}\nvar moved:str = resource.move()\nreturn 0\n}\n";
        assert!(inspect(src).is_ok());
    }

    #[test]
    fn 迴圈內借用後再明確移動資源仍須拒絕不穩定反向邊() {
        let src=format!("{SEE}fn main()->int{{\nvar owner:str = \"資源\"\nfor i in 1..4{{\nvar observed:int = see(owner)\nvar moved:str = owner.move()\n}}\nreturn 0\n}}\n");
        let err=inspect(&src).expect_err("函式借用退出不會容許多次迴圈移走持有者");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("反向邊"));
    }

    #[test]
    fn 分支內仍活躍的獨占借用不能因函式呼叫提前釋放() {
        let src="struct Resource{\nvar text:str = \"資源\"\nvar count:int = 1\n}\nfn see(value:Resource)->int{\nreturn value.count\n}\nfn main()->int{\nvar owner:Resource = Resource()\nvar alias:Resource = owner.borrow()\nif(true){\nvar observed:int = see(owner)\n}\nreturn alias.count\n}\n";
        let err=inspect(src).expect_err("分支不能把未來還要使用的別名提前解鎖");
        assert_eq!(err.kind, DiagnosticKind::Static);
        assert!(err.message.contains("HC14"));
    }

    #[test]
    fn 迭代區域內同名資源遮蔽不應轉移外層持有者() {
        let src=format!("{SEE}fn main()->int{{\nvar owner:str = \"外層\"\nfor i in 0..1{{\nvar owner:str = \"內層\"\nvar observed:int = see(owner)\n}}\nvar moved:str = owner.move()\nreturn 0\n}}\n");
        assert!(inspect(&src).is_ok());
    }
}

#[cfg(test)]
mod p16_真正_voil_持續結構整筆資源來源 {
    use super::trace_persistent_struct_move_source;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::DiagnosticKind;

    const VALID: &str="struct Message{\nvar title:str = \"預設\"\nconst id:str = \"固定🪼\"\n}\nfn main()->int{\nvar original:Message = Message(title=\"實際中文\")\nvar transferred:Message = original.move()\nreturn 0\n}\n";

    #[test]
    fn 真實_utf8_voil_具名覆蓋預設後只建兩個資源整筆移動逆序清理() {
        let (retained,events)=trace_persistent_struct_move_source(VALID,None).unwrap();
        assert_eq!(retained.as_deref(),Some("固定🪼"));
        assert_eq!(events,vec![
            ResourceEvent::Created(1),ResourceEvent::Created(2),
            ResourceEvent::Moved(1),ResourceEvent::Moved(2),
            ResourceEvent::Released(2),ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn 具名欄位完成後注入失敗_只回滾已求值字串() {
        let (retained,events)=trace_persistent_struct_move_source(VALID,Some("title")).unwrap();
        assert_eq!(retained,None);
        assert_eq!(events,vec![ResourceEvent::Created(1),ResourceEvent::Released(1)]);
    }

    #[test]
    fn 未覆蓋預設欄位失敗_具名和預設皆應逆序回滾() {
        let (retained,events)=trace_persistent_struct_move_source(VALID,Some("id")).unwrap();
        assert_eq!(retained,None);
        assert_eq!(events,vec![
            ResourceEvent::Created(1),ResourceEvent::Created(2),
            ResourceEvent::Released(2),ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn 不存在的失敗切點不能當作有效資源清理證據() {
        let error=trace_persistent_struct_move_source(VALID,Some("missing"))
            .expect_err("不存在求值計畫的欄位不得宣稱注入失敗");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("切點"));
    }

    #[test]
    fn 真正_voil_整體轉移禁止從來源不加_move_隱含複製() {
        let source=VALID.replace("original.move()","original");
        let error=trace_persistent_struct_move_source(&source,None)
            .expect_err("含 str 欄位的結構不是 Copyable");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("Copyable")||error.message.contains("複製"));
    }

    #[test]
    fn 真正_voil_整體轉移後不可第二次取用已移動來源() {
        let source=VALID.replace("return 0",
            "var again:Message = original.move()\nreturn 0");
        let error=trace_persistent_struct_move_source(&source,None)
            .expect_err("再次取用整體失效來源必須靜態拒絕");
        assert_eq!(error.kind,DiagnosticKind::Static);
    }

    #[test]
    fn 真正_voil_不允許個別移動_str_欄位取代整筆結構責任() {
        let source=VALID.replace("var transferred:Message = original.move()",
            "var transferred:str = original.title.move()");
        let error=trace_persistent_struct_move_source(&source,None)
            .expect_err("S21 禁止僅轉移資源欄位");
        assert_eq!(error.kind,DiagnosticKind::Static);
    }

    #[test]
    fn 靜態合法但多餘語句的來源不能假裝已由三步驟執行器完成() {
        let source=VALID.replace("return 0","var marker:int = 3\nreturn 0");
        let error=trace_persistent_struct_move_source(&source,None)
            .expect_err("一般多語句執行器仍未具備，不可虛報事件");
        assert_eq!(error.kind,DiagnosticKind::Static);
        assert!(error.message.contains("三語句"));
    }

    #[test]
    fn 尚未解碼的字串轉義不能直接寫成資源內容() {
        let source=VALID.replace("實際中文","實際\\n中文");
        let error=trace_persistent_struct_move_source(&source,None)
            .expect_err("沒有字串轉義求值器前不能偷採原始詞彙");
        assert!(error.message.contains("轉義")
            || matches!(error.kind,DiagnosticKind::Lexical|DiagnosticKind::Syntax));
    }
}

#[cfg(test)]
mod p16_真正_voil_複合結構返回資源責任 {
    use super::trace_persistent_struct_return_source;
    use crate::owned_str_runtime::ResourceEvent;
    use crate::DiagnosticKind;

    const SOURCE: &str="struct Bundle{\nvar title:str = \"預設\"\nconst id:str = \"固定🪼\"\n}\nfn provide()->Bundle{\nvar local:Bundle = Bundle(title=\"繁體中文\")\nreturn local.move()\n}\nfn main()->int{\nvar received:Bundle = provide()\nreturn 0\n}\n";

    #[test]
    fn 實際_voil_跨函式整筆返回原作用域不釋放_最終呼叫端逆序清理() {
        let (value,events)=trace_persistent_struct_return_source(
            SOURCE,"provide","received",None).unwrap();
        assert_eq!(value.as_deref(),Some("固定🪼"));
        assert_eq!(events,vec![
            ResourceEvent::Created(1),ResourceEvent::Created(2),
            ResourceEvent::Returned(1),ResourceEvent::Returned(2),
            ResourceEvent::Released(2),ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn 函式具名欄位求值失敗不發生任何返回事件() {
        let (value,events)=trace_persistent_struct_return_source(
            SOURCE,"provide","received",Some("title")).unwrap();
        assert_eq!(value,None);
        assert_eq!(events,vec![
            ResourceEvent::Created(1),ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn 函式預設欄位求值失敗僅由被呼叫端回滾已配置字串() {
        let (value,events)=trace_persistent_struct_return_source(
            SOURCE,"provide","received",Some("id")).unwrap();
        assert_eq!(value,None);
        assert_eq!(events,vec![
            ResourceEvent::Created(1),ResourceEvent::Created(2),
            ResourceEvent::Released(2),ResourceEvent::Released(1),
        ]);
    }

    #[test]
    fn 不能把非_copyable_結構一般名稱返回為第二個所有者() {
        let invalid=SOURCE.replace("return local.move()","return local");
        let err=trace_persistent_struct_return_source(
            &invalid,"provide","received",None)
            .expect_err("整體返回必須有資源責任轉移證明");
        assert_eq!(err.kind,DiagnosticKind::Static);
        assert!(err.message.contains("Copyable")||err.message.contains("複製"));
    }

    #[test]
    fn 非擁有借用不得由函式返回逃逸為複合結構所有者() {
        let invalid=SOURCE.replace("return local.move()","return local.borrow()");
        let err=trace_persistent_struct_return_source(
            &invalid,"provide","received",None)
            .expect_err("不能把整筆借用誤認成返回所有者");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }

    #[test]
    fn 被呼叫端尚未接通的一般多語句控制流程不能假裝已執行() {
        let invalid=SOURCE.replace("return local.move()",
            "var counter:int = 1\nreturn local.move()");
        let err=trace_persistent_struct_return_source(
            &invalid,"provide","received",None)
            .expect_err("函式多語句執行期尚未完成");
        assert_eq!(err.kind,DiagnosticKind::Static);
        assert!(err.message.contains("被呼叫"));
    }

    #[test]
    fn main_額外操作不應被資源模型悄悄略過() {
        let invalid=SOURCE.replace("return 0","var other:int = 1\nreturn 0");
        let err=trace_persistent_struct_return_source(
            &invalid,"provide","received",None)
            .expect_err("不可只模擬部分 main 再宣稱完整來源清理");
        assert_eq!(err.kind,DiagnosticKind::Static);
        assert!(err.message.contains("main"));
    }

    #[test]
    fn 返回函式字串原始轉義未經求值前保守拒絕() {
        let invalid=SOURCE.replace("繁體中文","繁體\\n中文");
        let err=trace_persistent_struct_return_source(
            &invalid,"provide","received",None)
            .expect_err("資源橋接沒有任意字串解碼器");
        assert!(err.message.contains("轉義")
            || matches!(err.kind,DiagnosticKind::Lexical|DiagnosticKind::Syntax));
    }
}

#[cfg(test)]
mod p16_真正_voil_結構欄位原子覆寫來源 {
    use super::{inspect,trace_persistent_struct_field_write_source};
    use crate::owned_str_runtime::ResourceEvent as E;
    use crate::DiagnosticKind;

    const SOURCE:&str="struct Holder{\nvar title:str = \"舊值\"\nconst id:str = \"固定🪼\"\n}\nfn main()->int{\nvar owner:Holder = Holder()\nowner.title = \"新值🪼\"\nreturn 0\n}\n";

    #[test]
    fn 真正_voil_右值建立後原子覆寫舊資源再逆序清理() {
        let (value,events)=trace_persistent_struct_field_write_source(SOURCE,false).unwrap();
        assert_eq!(value,"新值🪼");
        assert_eq!(events,vec![
            E::Created(1),E::Created(2),E::Created(3),
            E::Released(1),E::Released(3),E::Released(2),
        ]);
    }

    #[test]
    fn 真正_voil_右值故障只釋放暫存不更改舊持有者() {
        let (value,events)=trace_persistent_struct_field_write_source(SOURCE,true).unwrap();
        assert_eq!(value,"舊值");
        assert_eq!(events,vec![
            E::Created(1),E::Created(2),E::Created(3),
            E::Released(3),E::Released(2),E::Released(1),
        ]);
    }

    #[test]
    fn 真正_voil_沒有預設的_var_str_首次完成初始化不釋放假舊值() {
        let source=SOURCE.replace("var title:str = \"舊值\"","var title:str");
        let (value,events)=trace_persistent_struct_field_write_source(&source,false).unwrap();
        assert_eq!(value,"新值🪼");
        assert_eq!(events,vec![
            E::Created(1),E::Created(2),E::Released(2),E::Released(1),
        ]);
    }

    #[test]
    fn 真正_voil_const_結構實例即使欄位_var_也不可覆寫() {
        let source=SOURCE.replace("var owner:Holder","const owner:Holder");
        let err=trace_persistent_struct_field_write_source(&source,false)
            .expect_err("const 結構實例禁止任何欄位覆寫");
        assert_eq!(err.kind,DiagnosticKind::Static);
        assert!(err.message.contains("S18"));
    }

    #[test]
    fn 真正_voil_const_欄位在任何右值配置之前靜態拒絕() {
        let source=SOURCE.replace("owner.title = \"新值🪼\"","owner.id = \"新值🪼\"");
        let err=trace_persistent_struct_field_write_source(&source,false)
            .expect_err("const 欄位永遠不可覆寫");
        assert_eq!(err.kind,DiagnosticKind::Static);
        assert!(err.message.contains("S18"));
    }

    #[test]
    fn 真正_voil_非_copyable_具名右值不得充當隱含複製() {
        let source=SOURCE.replace("owner.title = \"新值🪼\"",
            "var borrowed:str = \"其他\"\nowner.title = borrowed");
        let err=inspect(&source).expect_err("str 右值非 Copyable 且沒有明確移動");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }

    #[test]
    fn 真正_voil_來源作用中借用阻止結構直接欄位覆寫() {
        let source=SOURCE.replace("owner.title = \"新值🪼\"",
            "const alias:Holder = owner.borrow()\nowner.title = \"新值🪼\"\nvar later:int = 1");
        let err=trace_persistent_struct_field_write_source(&source,false)
            .expect_err("借用有效期資料流或非三語句執行證明不得放行");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }

    #[test]
    fn 真正_voil_非法欄位與不相關控制語句不得被省略執行() {
        let bad=SOURCE.replace("owner.title = \"新值🪼\"","owner.unknown = \"新值🪼\"");
        assert_eq!(inspect(&bad).unwrap_err().kind,DiagnosticKind::Static);
        let extra=SOURCE.replace("return 0","var x:int = 1\nreturn 0");
        let err=trace_persistent_struct_field_write_source(&extra,false)
            .expect_err("未實作任意語句執行器前不能宣稱清理已完整");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }

    #[test]
    fn 真正_voil_帶轉義字串不能被當成未轉義_utf8_內容() {
        let source=SOURCE.replace("新值🪼","帶\\n轉義");
        let err=trace_persistent_struct_field_write_source(&source,false)
            .expect_err("未有字串解碼器不接受此來源橋接");
        assert!(err.message.contains("轉義")
            || matches!(err.kind,DiagnosticKind::Lexical|DiagnosticKind::Syntax));
    }
}

#[cfg(test)]
mod p16_真正_voil_混合純值與_utf8_結構來源 {
    use super::trace_mixed_struct_source;
    use crate::owned_str_runtime::ResourceEvent as E;
    use crate::DiagnosticKind;

    const MIXED:&str="struct Mixed{\nvar num:int = 3\nvar text:str = \"舊值\"\nconst flag:bool = true\n}\nfn main()->int{\nvar original:Mixed = Mixed(text=\"繁體中文🪼\",num=7)\nvar target:Mixed = original.move()\nreturn 0\n}\n";
    const PLAIN:&str="struct Plain{\nvar num:int = 3\nconst flag:bool = true\n}\nfn main()->int{\nvar original:Plain = Plain(num=9)\nvar target:Plain = original\nreturn 0\n}\n";

    #[test]
    fn 真正_voil_混合結構具名優先求值只有字串建立移動清理事件() {
        let (value,events)=trace_mixed_struct_source(MIXED,None).unwrap();
        assert_eq!(value,Some(7));
        assert_eq!(events,vec![E::Created(1),E::Moved(1),E::Released(1)]);
    }

    #[test]
    fn 真正_voil_全_copyable_結構整體複製不虛構資源事件() {
        let (value,events)=trace_mixed_struct_source(PLAIN,None).unwrap();
        assert_eq!(value,Some(9));
        assert!(events.is_empty());
    }

    #[test]
    fn 真正_voil_混合結構隱含複製必須在資源分配前靜態拒絕() {
        let illegal=MIXED.replace("original.move()","original");
        let err=trace_mixed_struct_source(&illegal,None)
            .expect_err("str 欄位不得由隱含複製變成第二所有者");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }

    #[test]
    fn 真正_voil_混合具名_str_成功後後續_int_失敗只逆序清理資源() {
        let (value,events)=trace_mixed_struct_source(MIXED,Some("num")).unwrap();
        assert_eq!(value,None);
        assert_eq!(events,vec![E::Created(1),E::Released(1)]);
    }

    #[test]
    fn 真正_voil_混合預設_bool_失敗不假造純值資源() {
        let (value,events)=trace_mixed_struct_source(MIXED,Some("flag")).unwrap();
        assert_eq!(value,None);
        assert_eq!(events,vec![E::Created(1),E::Released(1)]);
    }

    #[test]
    fn 真正_voil_沒有執行的覆蓋預設欄位不能充當失敗注入切點() {
        let err=trace_mixed_struct_source(MIXED,Some("unknown"))
            .expect_err("不存在的求值步驟不得偽裝發生資源故障");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }

    #[test]
    fn 真正_voil_尚未初始化的_str_var_不能被整筆轉移() {
        let invalid=MIXED.replace("var text:str = \"舊值\"","var text:str")
            .replace("text=\"繁體中文🪼\",","").replace("Mixed(,","Mixed(");
        let err=trace_mixed_struct_source(&invalid,None)
            .expect_err("部分初始化不可當完整資源所有者移動");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }

    #[test]
    fn 真正_voil_未經核准的混合複合求值不能假稱已執行() {
        let extra=MIXED.replace("return 0","var extra:int = 1\nreturn 0");
        let err=trace_mixed_struct_source(&extra,None)
            .expect_err("沒有一般多語句執行模型前不得虛報清理");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p16_真實_voil_分支提前返回資源清理 {
    use super::trace_scope_struct_source;
    use crate::owned_str_runtime::ResourceEvent as E;
    use crate::DiagnosticKind;

    const SOURCE:&str="struct Holder{\nvar text:str = \"預設\"\n}\nfn main()->int{\nvar outer:Holder = Holder(text=\"外層\")\nif(true){\nvar inner:Holder = Holder(text=\"內層\")\n}\nreturn 0\n}\n";
    const CLEAN:&[E]=&[
        E::Created(1),E::Created(2),E::Released(2),E::Released(1),
    ];

    #[test]
    fn 真正_voil_if_true_內層先清理_外層最後清理() {
        assert_eq!(trace_scope_struct_source(SOURCE,None).unwrap(),CLEAN);
    }
    #[test]
    fn 真正_voil_if_false_有_else_只配置實際執行的分支() {
        let source=SOURCE.replace("if(true){\nvar inner:Holder = Holder(text=\"內層\")\n}",
            "if(false){\nvar inner:Holder = Holder(text=\"未執行\")\n}else{\nvar other:Holder = Holder(text=\"執行\")\n}");
        assert_eq!(trace_scope_struct_source(&source,None).unwrap(),CLEAN);
    }
    #[test]
    fn 真正_voil_if_false_無_else_不能分配虛構的內層資源() {
        let source=SOURCE.replace("if(true)","if(false)");
        assert_eq!(trace_scope_struct_source(&source,None).unwrap(),vec![
            E::Created(1),E::Released(1),
        ]);
    }
    #[test]
    fn 真正_voil_內層資源初始化後故障須先回滾再清理外層() {
        assert_eq!(trace_scope_struct_source(SOURCE,Some("text")).unwrap(),CLEAN);
    }
    #[test]
    fn 真正_voil_if_分支內提前_return_離開函式按逆序清理() {
        let source=SOURCE.replace(
            "var inner:Holder = Holder(text=\"內層\")\n}",
            "var inner:Holder = Holder(text=\"內層\")\nreturn 0\n}");
        assert_eq!(trace_scope_struct_source(&source,None).unwrap(),CLEAN);
    }
    #[test]
    fn 真正_voil_if_分支提前返回且故障時不得遺失外層() {
        let source=SOURCE.replace(
            "var inner:Holder = Holder(text=\"內層\")\n}",
            "var inner:Holder = Holder(text=\"內層\")\nreturn 0\n}");
        assert_eq!(trace_scope_struct_source(&source,Some("text")).unwrap(),CLEAN);
    }
    #[test]
    fn 真正_voil_未知條件不能被當作已確定執行的分支() {
        let source=SOURCE.replace("if(true)","if(1==1)");
        let err=trace_scope_struct_source(&source,None)
            .expect_err("沒有布林值運算式執行器不能造假清理來源");
        assert!(matches!(err.kind,DiagnosticKind::Static|
            DiagnosticKind::Syntax|DiagnosticKind::Lexical));
    }
    #[test]
    fn 真正_voil_分支中的額外副作用不能被跳過仍宣稱完整資源清理() {
        let source=SOURCE.replace(
            "var inner:Holder = Holder(text=\"內層\")",
            "var inner:Holder = Holder(text=\"內層\")\nvar x:int = 2");
        let err=trace_scope_struct_source(&source,None)
            .expect_err("尚未接通的額外語句不能被模型忽略");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }
    #[test]
    fn 真正_voil_不執行的分支不能注入不存在的失敗事件() {
        let source=SOURCE.replace("if(true)","if(false)");
        let err=trace_scope_struct_source(&source,Some("text"))
            .expect_err("不得將未執行的分支資源當成已配置");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }
    #[test]
    fn 真正_voil_分支內未支援的多層嵌套不能直接推定清理() {
        let source=SOURCE.replace(
            "var inner:Holder = Holder(text=\"內層\")",
            "if(true){\nvar inner:Holder = Holder(text=\"內層\")\n}");
        let err=trace_scope_struct_source(&source,None)
            .expect_err("不能把未模擬的嵌套資源當已追蹤");
        assert_eq!(err.kind,DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p16_6_r03_來源安全事實一致性 {
    use super::*;

    #[test]
    fn 私有公開入口和資源橋接使用同一份已驗證_ast() {
        let valid = "fn main()->int{\nvar a:int = 1\nreturn a\n}\n";
        assert!(inspect(valid).is_ok());
        let verified = verified_private_source(valid).expect("私有來源須完整通過驗證");
        assert_eq!(verified.functions.len(), 1);
        assert_eq!(verified.functions[0].name, "main");
        assert!(verified.structs.is_empty());
        assert_eq!(verified.source.bytes, valid.as_bytes());
    }

    #[test]
    fn 非_copyable_隱含複製不得在資源橋接跳過安全預檢() {
        let invalid = "fn main()->int{\nvar a:str = \"中文\"\nvar b:str = a\nreturn 0\n}\n";
        let baseline = inspect(invalid).err().expect("私有入口必須拒絕");
        let bridge = verified_private_source(invalid).err()
            .expect("資源執行期的前置驗證必須同樣拒絕");
        assert_eq!(baseline, bridge);
        assert_eq!(bridge.kind, DiagnosticKind::Static);
        assert!(bridge.span.is_some());
    }
}

#[cfg(test)]
mod p16_6_r04_私有作用域快照回歸 {
    use super::*;

    #[test]
    fn 借用和移動的分支快照寫入不能污染來源() {
        let mut original_map = HashMap::new();
        original_map.insert("text".to_owned(), Binding {
            declaration_start: 11,
            ty: "str".to_owned(),
            mutable: true,
            value: ValueSlot::initialized(CopyPermission::Forbidden),
            pending_fields: None,
            loan: None,
        });
        let initial: Scopes = vec![Rc::new(original_map)];
        let mut moved = initial.clone();
        assert!(Rc::ptr_eq(&initial[0], &moved[0]));
        let old = Rc::make_mut(&mut moved[0]).get_mut("text").unwrap();
        assert!(old.value.move_whole().is_ok());
        assert!(!Rc::ptr_eq(&initial[0], &moved[0]));
        assert!(initial[0]["text"].value.read().is_ok());
        assert!(moved[0]["text"].value.read().is_err());
        assert_eq!(initial[0]["text"].declaration_start, moved[0]["text"].declaration_start);
    }

    #[test]
    fn 原已通過的迴圈借用與提前轉移安全來源仍保留() {
        let valid = "fn main()->int{\nvar x:int = 1\nfor i in 0..1{\nx = 2\n}\nreturn x\n}\n";
        assert!(inspect(valid).is_ok());
        let invalid = "fn main()->int{\nvar x:str = \"a\"\nif (true){\nvar moved:str = x.move()\n}else{\nreturn 0\n}\nvar again:str = x.move()\nreturn 0\n}\n";
        assert_eq!(inspect(invalid).err().unwrap().kind, DiagnosticKind::Static);
    }
}

#[cfg(test)]
mod p16_6_r04_不必要作用域複製防回歸 {
    use super::*;

    fn binding(start: usize, loan: Option<LoanBinding>) -> Binding {
        Binding {
            declaration_start: start,
            ty: "int".to_owned(),
            mutable: true,
            value: ValueSlot::initialized(CopyPermission::Allowed),
            pending_fields: None,
            loan,
        }
    }

    #[test]
    fn 無活躍借用與參數借用無須複製任何共享作用域() {
        let mut source = HashMap::new();
        for index in 0..96 {
            source.insert(format!("value_{index}"), binding(index + 100, None));
        }
        source.insert("parameter".to_owned(), binding(20, Some(LoanBinding {
            source: 20, kind: BorrowKind::SharedRead, active: true,
        })));
        let original = Rc::new(source);
        let mut vars: Scopes = vec![original.clone()];
        Checker::<fn(&str) -> CopyPermission>::release_last_uses(
            &mut vars, &[], &[]);
        assert!(Rc::ptr_eq(&original, &vars[0]));
        assert!(vars[0]["parameter"].loan.as_ref().unwrap().active);
    }

    #[test]
    fn 真正來源借用仍在最後使用點歸還且不污染快照() {
        let mut source = HashMap::new();
        source.insert("alias".to_owned(), binding(40, Some(LoanBinding {
            source: 10, kind: BorrowKind::ExclusiveWrite, active: true,
        })));
        let original = Rc::new(source);
        let mut vars: Scopes = vec![original.clone()];
        Checker::<fn(&str) -> CopyPermission>::release_last_uses(
            &mut vars, &[], &[]);
        assert!(!Rc::ptr_eq(&original, &vars[0]));
        assert!(original["alias"].loan.as_ref().unwrap().active);
        assert!(!vars[0]["alias"].loan.as_ref().unwrap().active);
    }
}
